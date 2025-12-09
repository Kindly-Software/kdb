//! Comprehensive tests for temporal module
//!
//! Implements T42 framework testing temporal arbitrage opportunities,
//! confidence handling, and timing validation.

use fractal_arbitrage_scanner::{TemporalArbitrageOpportunity, aid_class};
use std::time::Duration;

/// T42.1: Test basic temporal opportunity creation
#[test]
fn test_temporal_opportunity_creation() {
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.75,
        Duration::from_millis(100),
    );

    // Verify all fields are set correctly
    assert_eq!(temporal_op.symbol, "BTC/USD");
    assert_eq!(temporal_op.current_price, 50_000.0);
    assert_eq!(temporal_op.future_price, 51_000.0);
    assert_eq!(temporal_op.confidence, 0.75);
    assert_eq!(temporal_op.execution_delay, Duration::from_millis(100));

    // Verify AID-96 class is correct
    assert_eq!(temporal_op.id.class(), aid_class::ALT);
}

/// T42.2: Test confidence clamping to [0.0, 1.0]
#[test]
fn test_confidence_clamping() {
    // Test confidence > 1.0 gets clamped to 1.0
    let temporal_op = TemporalArbitrageOpportunity::new(
        "ETH/USD",
        3_000.0,
        3_500.0,
        1.5, // Should be clamped to 1.0
        Duration::from_millis(50),
    );
    assert_eq!(temporal_op.confidence, 1.0);

    // Test confidence < 0.0 gets clamped to 0.0
    let temporal_op = TemporalArbitrageOpportunity::new(
        "ETH/USD",
        3_000.0,
        3_500.0,
        -0.3, // Should be clamped to 0.0
        Duration::from_millis(50),
    );
    assert_eq!(temporal_op.confidence, 0.0);

    // Test exactly 0.0 and 1.0
    let temporal_op = TemporalArbitrageOpportunity::new(
        "ETH/USD",
        3_000.0,
        3_500.0,
        0.0,
        Duration::from_millis(50),
    );
    assert_eq!(temporal_op.confidence, 0.0);

    let temporal_op = TemporalArbitrageOpportunity::new(
        "ETH/USD",
        3_000.0,
        3_500.0,
        1.0,
        Duration::from_millis(50),
    );
    assert_eq!(temporal_op.confidence, 1.0);
}

/// T42.3: Test extreme confidence values
#[test]
fn test_extreme_confidence_values() {
    // Test very large positive confidence
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        60_000.0,
        1_000_000.0, // Extremely large
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.confidence, 1.0);

    // Test very large negative confidence
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        60_000.0,
        -1_000_000.0, // Extremely negative
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.confidence, 0.0);

    // Test NaN confidence
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        60_000.0,
        f64::NAN,
        Duration::from_millis(100),
    );
    // NaN should be clamped to 0.0 (based on typical clamp behavior)
    assert_eq!(temporal_op.confidence, 0.0);

    // Test infinity confidence
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        60_000.0,
        f64::INFINITY,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.confidence, 1.0);

    // Test negative infinity confidence
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        60_000.0,
        f64::NEG_INFINITY,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.confidence, 0.0);
}

/// T42.4: Test various execution delays
#[test]
fn test_execution_delays() {
    // Test zero delay
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.8,
        Duration::from_nanos(0),
    );
    assert_eq!(temporal_op.execution_delay, Duration::from_nanos(0));

    // Test nanosecond precision
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.8,
        Duration::from_nanos(1),
    );
    assert_eq!(temporal_op.execution_delay, Duration::from_nanos(1));

    // Test microseconds
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.8,
        Duration::from_micros(500),
    );
    assert_eq!(temporal_op.execution_delay, Duration::from_micros(500));

    // Test milliseconds
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.8,
        Duration::from_millis(250),
    );
    assert_eq!(temporal_op.execution_delay, Duration::from_millis(250));

    // Test seconds
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.8,
        Duration::from_secs(5),
    );
    assert_eq!(temporal_op.execution_delay, Duration::from_secs(5));

    // Test very large delay
    let temporal_op = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.8,
        Duration::from_secs(86400), // 1 day
    );
    assert_eq!(temporal_op.execution_delay, Duration::from_secs(86400));
}

/// T42.5: Test different price scenarios
#[test]
fn test_price_scenarios() {
    // Test price increase
    let temporal_op = TemporalArbitrageOpportunity::new(
        "ETH/USD",
        3_000.0,
        3_200.0, // Increase
        0.7,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.current_price, 3_000.0);
    assert_eq!(temporal_op.future_price, 3_200.0);

    // Test price decrease
    let temporal_op = TemporalArbitrageOpportunity::new(
        "ETH/USD",
        3_000.0,
        2_800.0, // Decrease
        0.6,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.current_price, 3_000.0);
    assert_eq!(temporal_op.future_price, 2_800.0);

    // Test no price change
    let temporal_op = TemporalArbitrageOpportunity::new(
        "ETH/USD",
        3_000.0,
        3_000.0, // No change
        0.5,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.current_price, 3_000.0);
    assert_eq!(temporal_op.future_price, 3_000.0);

    // Test very small prices
    let temporal_op = TemporalArbitrageOpportunity::new(
        "TINY/USD",
        0.000001,
        0.000002,
        0.9,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.current_price, 0.000001);
    assert_eq!(temporal_op.future_price, 0.000002);

    // Test very large prices
    let temporal_op = TemporalArbitrageOpportunity::new(
        "HUGE/USD",
        1_000_000.0,
        1_100_000.0,
        0.8,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.current_price, 1_000_000.0);
    assert_eq!(temporal_op.future_price, 1_100_000.0);
}

/// T42.6: Test edge case prices
#[test]
fn test_edge_case_prices() {
    use std::f64::consts::{E, PI};

    // Test irrational numbers
    let temporal_op = TemporalArbitrageOpportunity::new(
        "PI/USD",
        PI,
        E,
        0.618, // Golden ratio confidence
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.current_price, PI);
    assert_eq!(temporal_op.future_price, E);

    // Test extremely small positive numbers
    let temporal_op = TemporalArbitrageOpportunity::new(
        "TINY/USD",
        f64::MIN_POSITIVE,
        f64::MIN_POSITIVE * 2.0,
        0.5,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.current_price, f64::MIN_POSITIVE);
    assert_eq!(temporal_op.future_price, f64::MIN_POSITIVE * 2.0);

    // Test very large numbers (but finite)
    let large_price = 1e100;
    let temporal_op = TemporalArbitrageOpportunity::new(
        "LARGE/USD",
        large_price,
        large_price * 1.01,
        0.5,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.current_price, large_price);
    assert_eq!(temporal_op.future_price, large_price * 1.01);
}

/// T42.7: Test symbol handling
#[test]
fn test_symbol_handling() {
    // Test standard symbols
    let symbols = vec![
        "BTC/USD",
        "ETH/BTC",
        "XRP/EUR",
        "ADA/USDT",
        "DOT/USDC",
    ];

    for symbol in symbols {
        let temporal_op = TemporalArbitrageOpportunity::new(
            symbol,
            1000.0,
            1010.0,
            0.5,
            Duration::from_millis(100),
        );
        assert_eq!(temporal_op.symbol, symbol);
    }

    // Test unusual symbols
    let temporal_op = TemporalArbitrageOpportunity::new(
        "LONG_SYMBOL_NAME/ANOTHER_LONG_NAME",
        1000.0,
        1010.0,
        0.5,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.symbol, "LONG_SYMBOL_NAME/ANOTHER_LONG_NAME");

    // Test symbols with numbers
    let temporal_op = TemporalArbitrageOpportunity::new(
        "TOKEN123/USD456",
        1000.0,
        1010.0,
        0.5,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.symbol, "TOKEN123/USD456");

    // Test empty symbol (edge case)
    let temporal_op = TemporalArbitrageOpportunity::new(
        "",
        1000.0,
        1010.0,
        0.5,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.symbol, "");

    // Test single character symbol
    let temporal_op = TemporalArbitrageOpportunity::new(
        "X",
        1000.0,
        1010.0,
        0.5,
        Duration::from_millis(100),
    );
    assert_eq!(temporal_op.symbol, "X");
}

/// T42.8: Test multiple opportunities have unique IDs
#[test]
fn test_unique_id_generation() {
    let mut ids = std::collections::HashSet::new();

    // Generate many temporal opportunities
    for i in 0..1000 {
        let temporal_op = TemporalArbitrageOpportunity::new(
            &format!("SYMBOL{}", i),
            1000.0 + i as f64,
            1010.0 + i as f64,
            0.5,
            Duration::from_millis(100),
        );

        // Each ID should be unique
        assert!(
            ids.insert(temporal_op.id),
            "Duplicate ID generated: {:?}",
            temporal_op.id
        );

        // All should have ALT class
        assert_eq!(temporal_op.id.class(), aid_class::ALT);
    }

    // Should have 1000 unique IDs
    assert_eq!(ids.len(), 1000);
}

/// T42.9: Test serialization and deserialization
#[test]
fn test_serde() {
    let original = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.75,
        Duration::from_millis(125),
    );

    // Test JSON serialization
    let serialized = serde_json::to_string(&original)
        .expect("Should serialize to JSON");

    let deserialized: TemporalArbitrageOpportunity = serde_json::from_str(&serialized)
        .expect("Should deserialize from JSON");

    // All fields should be equal
    assert_eq!(original, deserialized);
    assert_eq!(original.id, deserialized.id);
    assert_eq!(original.symbol, deserialized.symbol);
    assert_eq!(original.current_price, deserialized.current_price);
    assert_eq!(original.future_price, deserialized.future_price);
    assert_eq!(original.confidence, deserialized.confidence);
    assert_eq!(original.execution_delay, deserialized.execution_delay);
}

/// T42.10: Test equality and inequality
#[test]
fn test_equality() {
    let temporal_op1 = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.75,
        Duration::from_millis(100),
    );

    let temporal_op2 = TemporalArbitrageOpportunity::new(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.75,
        Duration::from_millis(100),
    );

    // Different instances with same data should not be equal (due to unique IDs)
    assert_ne!(temporal_op1, temporal_op2);
    assert_ne!(temporal_op1.id, temporal_op2.id);

    // But other fields should be equal
    assert_eq!(temporal_op1.symbol, temporal_op2.symbol);
    assert_eq!(temporal_op1.current_price, temporal_op2.current_price);
    assert_eq!(temporal_op1.future_price, temporal_op2.future_price);
    assert_eq!(temporal_op1.confidence, temporal_op2.confidence);
    assert_eq!(temporal_op1.execution_delay, temporal_op2.execution_delay);

    // Test clone equality
    let cloned = temporal_op1.clone();
    assert_eq!(temporal_op1, cloned);
}

/// T42.11: Test debug and display formatting
#[test]
fn test_formatting() {
    let temporal_op = TemporalArbitrageOpportunity::new(
        "ETH/USD",
        3_000.0,
        3_100.0,
        0.85,
        Duration::from_millis(150),
    );

    // Test debug formatting
    let debug_str = format!("{:?}", temporal_op);
    assert!(debug_str.contains("TemporalArbitrageOpportunity"));
    assert!(debug_str.contains("ETH/USD"));
    assert!(debug_str.contains("3000"));
    assert!(debug_str.contains("3100"));
    assert!(debug_str.contains("0.85"));

    // Test that we can format the ID
    let id_debug = format!("{:?}", temporal_op.id);
    assert!(!id_debug.is_empty());
}

/// T42.12: Property-based test for confidence invariants
#[test]
fn test_confidence_invariants() {
    let test_cases = vec![
        -f64::INFINITY,
        -1000.0,
        -1.0,
        -0.5,
        0.0,
        0.25,
        0.5,
        0.75,
        1.0,
        1.5,
        2.0,
        1000.0,
        f64::INFINITY,
        f64::NAN,
    ];

    for confidence in test_cases {
        let temporal_op = TemporalArbitrageOpportunity::new(
            "TEST/USD",
            100.0,
            110.0,
            confidence,
            Duration::from_millis(100),
        );

        // Confidence should always be in [0.0, 1.0] and finite
        assert!(temporal_op.confidence >= 0.0);
        assert!(temporal_op.confidence <= 1.0);
        assert!(temporal_op.confidence.is_finite());
    }
}

/// T42.13: Test concurrent creation of temporal opportunities
#[test]
fn test_concurrent_creation() {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::thread;

    let ids = Arc::new(Mutex::new(std::collections::HashSet::new()));
    let mut handles = vec![];

    // Spawn multiple threads creating opportunities
    for i in 0..10 {
        let ids_clone = Arc::clone(&ids);
        let handle = thread::spawn(move || {
            let mut local_ids = vec![];
            for j in 0..100 {
                let temporal_op = TemporalArbitrageOpportunity::new(
                    &format!("COIN{}/USD", i),
                    1000.0 + (i * 100 + j) as f64,
                    1010.0 + (i * 100 + j) as f64,
                    0.5,
                    Duration::from_millis(100),
                );
                local_ids.push(temporal_op.id);
            }

            // Add all local IDs to the global set
            let mut global_ids = ids_clone.lock().unwrap();
            for id in local_ids {
                assert!(global_ids.insert(id), "Duplicate ID generated in concurrent test");
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Should have 1000 unique IDs (10 threads * 100 opportunities each)
    let final_ids = ids.lock().unwrap();
    assert_eq!(final_ids.len(), 1000);
}