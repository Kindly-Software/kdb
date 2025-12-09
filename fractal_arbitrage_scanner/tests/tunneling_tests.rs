//! Comprehensive tests for tunneling integration module
//!
//! Implements T42 framework testing tunneling scanner, barrier types,
//! and quantum tunneling probability calculations.

use fractal_arbitrage_scanner::{aid_class, TunnelingOpportunity, TunnelingScanner};
use fractal_arbitrage_scanner::tunneling_integration::BarrierType;

/// T42.1: Test TunnelingScanner creation and initialization
#[test]
fn test_tunneling_scanner_creation() {
    let scanner = TunnelingScanner::new(42);
    // Scanner should be created successfully

    let default_scanner = TunnelingScanner::default();
    // Default scanner should also work

    // Test that both scanners can derive opportunities
    let opportunity = scanner.derive_opportunity("BTC/USD", 50_000.0, 51_000.0);
    assert_eq!(opportunity.node_hint, 42);

    let default_opportunity = default_scanner.derive_opportunity("ETH/USD", 3_000.0, 3_100.0);
    assert_eq!(default_opportunity.node_hint, 0); // Default node hint
}

/// T42.2: Test resistance barrier detection
#[test]
fn test_resistance_barrier() {
    let scanner = TunnelingScanner::new(1337);

    // Barrier above current price = resistance
    let opportunity = scanner.derive_opportunity(
        "BTC/USD",
        50_000.0, // current_price
        51_000.0, // barrier_price (higher = resistance)
    );

    assert_eq!(opportunity.barrier_type, BarrierType::Resistance);
    assert_eq!(opportunity.symbol, "BTC/USD");
    assert_eq!(opportunity.current_price, 50_000.0);
    assert_eq!(opportunity.barrier_price, 51_000.0);
    assert_eq!(opportunity.node_hint, 1337);
    assert_eq!(opportunity.transmission_probability, 0.5);

    // Verify AID-96 class
    assert_eq!(opportunity.id.class(), aid_class::DOS);

    // Verify profit calculation for resistance
    let expected_profit_bp = (((51_000.0 - 50_000.0) / 50_000.0) * 10_000.0_f64).abs() as u32;
    assert_eq!(opportunity.expected_profit_bp, expected_profit_bp);
    assert_eq!(opportunity.expected_profit_bp, 200); // 2% = 200 basis points
}

/// T42.3: Test support barrier detection
#[test]
fn test_support_barrier() {
    let scanner = TunnelingScanner::new(42);

    // Barrier below current price = support
    let opportunity = scanner.derive_opportunity(
        "ETH/USD",
        3_000.0, // current_price
        2_900.0, // barrier_price (lower = support)
    );

    assert_eq!(opportunity.barrier_type, BarrierType::Support);
    assert_eq!(opportunity.symbol, "ETH/USD");
    assert_eq!(opportunity.current_price, 3_000.0);
    assert_eq!(opportunity.barrier_price, 2_900.0);
    assert_eq!(opportunity.node_hint, 42);

    // Verify profit calculation uses absolute value
    let expected_profit_bp = (((2_900.0 - 3_000.0) / 3_000.0) * 10_000.0_f64).abs() as u32;
    assert_eq!(opportunity.expected_profit_bp, expected_profit_bp);
    assert_eq!(opportunity.expected_profit_bp, 333); // ~3.33% = 333 basis points
}

/// T42.4: Test barrier at exactly current price
#[test]
fn test_barrier_at_current_price() {
    let scanner = TunnelingScanner::new(100);

    // Barrier equal to current price
    let opportunity = scanner.derive_opportunity(
        "BTC/USD",
        50_000.0, // current_price
        50_000.0, // barrier_price (equal)
    );

    // Equal prices should be treated as resistance (>= condition)
    assert_eq!(opportunity.barrier_type, BarrierType::Resistance);
    assert_eq!(opportunity.current_price, 50_000.0);
    assert_eq!(opportunity.barrier_price, 50_000.0);

    // Zero price difference should result in zero profit
    assert_eq!(opportunity.expected_profit_bp, 0);
}

/// T42.5: Test various node hints
#[test]
fn test_node_hints() {
    let test_cases = vec![0, 1, 42, 255, 1337, 65535];

    for node_hint in test_cases {
        let scanner = TunnelingScanner::new(node_hint);
        let opportunity = scanner.derive_opportunity("TEST/USD", 100.0, 110.0);

        assert_eq!(opportunity.node_hint, node_hint);
        // Other fields should be consistent regardless of node hint
        assert_eq!(opportunity.barrier_type, BarrierType::Resistance);
        assert_eq!(opportunity.transmission_probability, 0.5);
    }
}

/// T42.6: Test profit calculation edge cases
#[test]
fn test_profit_calculation_edge_cases() {
    let scanner = TunnelingScanner::new(42);

    // Very small spread
    let opportunity = scanner.derive_opportunity("BTC/USD", 50_000.0, 50_000.01);
    let expected_tiny = (((50_000.01 - 50_000.0) / 50_000.0) * 10_000.0_f64).abs() as u32;
    assert_eq!(opportunity.expected_profit_bp, expected_tiny);
    assert_eq!(opportunity.expected_profit_bp, 0); // Rounds to 0

    // Large spread
    let opportunity = scanner.derive_opportunity("VOLATILE/USD", 100.0, 200.0);
    let expected_large = (((200.0 - 100.0) / 100.0) * 10_000.0_f64).abs() as u32;
    assert_eq!(opportunity.expected_profit_bp, expected_large);
    assert_eq!(opportunity.expected_profit_bp, 10_000); // 100% = 10,000 basis points

    // Very small prices
    let opportunity = scanner.derive_opportunity("MICRO/USD", 0.000001, 0.000002);
    let expected_micro = (((0.000002 - 0.000001) / 0.000001) * 10_000.0_f64).abs() as u32;
    assert_eq!(opportunity.expected_profit_bp, expected_micro);
    assert_eq!(opportunity.expected_profit_bp, 10_000); // 100% spread

    // Large prices
    let opportunity = scanner.derive_opportunity("LARGE/USD", 1_000_000.0, 1_010_000.0);
    let expected_big = (((1_010_000.0 - 1_000_000.0) / 1_000_000.0) * 10_000.0_f64).abs() as u32;
    assert_eq!(opportunity.expected_profit_bp, expected_big);
    assert_eq!(opportunity.expected_profit_bp, 100); // 1% = 100 basis points
}

/// T42.7: Test extreme price scenarios
#[test]
fn test_extreme_price_scenarios() {
    let scanner = TunnelingScanner::new(42);

    // Test with very small positive numbers
    let opportunity = scanner.derive_opportunity(
        "TINY/USD",
        f64::MIN_POSITIVE,
        f64::MIN_POSITIVE * 2.0,
    );
    assert!(opportunity.expected_profit_bp < u32::MAX); // Should not overflow

    // Test with very large finite numbers
    let large_price = 1e10;
    let opportunity = scanner.derive_opportunity(
        "HUGE/USD",
        large_price,
        large_price * 1.001,
    );
    assert_eq!(opportunity.expected_profit_bp, 10); // 0.1% = 10 basis points

    // Test negative price difference (support case)
    let opportunity = scanner.derive_opportunity("TEST/USD", 1000.0, 900.0);
    assert_eq!(opportunity.barrier_type, BarrierType::Support);
    assert_eq!(opportunity.expected_profit_bp, 1000); // 10% = 1000 basis points
}

/// T42.8: Test different symbols
#[test]
fn test_symbol_handling() {
    let scanner = TunnelingScanner::new(42);

    let symbols = vec![
        "BTC/USD",
        "ETH/BTC",
        "XRP/EUR",
        "ADA/USDT",
        "DOT/USDC",
        "LONG_SYMBOL_NAME/ANOTHER_LONG_NAME",
        "123TOKEN/456COIN",
        "",
        "X",
    ];

    for symbol in symbols {
        let opportunity = scanner.derive_opportunity(symbol, 100.0, 110.0);
        assert_eq!(opportunity.symbol, symbol);
        // Other fields should be consistent
        assert_eq!(opportunity.barrier_type, BarrierType::Resistance);
        assert_eq!(opportunity.transmission_probability, 0.5);
    }
}

/// T42.9: Test unique ID generation
#[test]
fn test_unique_id_generation() {
    let scanner = TunnelingScanner::new(42);
    let mut ids = std::collections::HashSet::new();

    // Generate many opportunities
    for i in 0..1000 {
        let opportunity = scanner.derive_opportunity(
            &format!("SYMBOL{}", i),
            1000.0 + i as f64,
            1100.0 + i as f64,
        );

        // Each ID should be unique
        assert!(
            ids.insert(opportunity.id),
            "Duplicate ID generated: {:?}",
            opportunity.id
        );

        // All should have DOS class
        assert_eq!(opportunity.id.class(), aid_class::DOS);
    }

    // Should have 1000 unique IDs
    assert_eq!(ids.len(), 1000);
}

/// T42.10: Test BarrierType enum properties
#[test]
fn test_barrier_type_enum() {
    // Test equality
    assert_eq!(BarrierType::Resistance, BarrierType::Resistance);
    assert_eq!(BarrierType::Support, BarrierType::Support);
    assert_ne!(BarrierType::Resistance, BarrierType::Support);

    // Test debug formatting
    let resistance_debug = format!("{:?}", BarrierType::Resistance);
    let support_debug = format!("{:?}", BarrierType::Support);
    assert_eq!(resistance_debug, "Resistance");
    assert_eq!(support_debug, "Support");

    // Test cloning
    let original = BarrierType::Resistance;
    let cloned = original.clone();
    assert_eq!(original, cloned);

    // Test copying (BarrierType implements Copy)
    let copied = original;
    assert_eq!(original, copied);
}

/// T42.11: Test TunnelingOpportunity serialization
#[test]
fn test_tunneling_opportunity_serde() {
    let scanner = TunnelingScanner::new(1337);
    let original = scanner.derive_opportunity("BTC/USD", 50_000.0, 51_000.0);

    // Test JSON serialization
    let serialized = serde_json::to_string(&original)
        .expect("Should serialize to JSON");

    let deserialized: TunnelingOpportunity = serde_json::from_str(&serialized)
        .expect("Should deserialize from JSON");

    // All fields should be equal
    assert_eq!(original, deserialized);
    assert_eq!(original.id, deserialized.id);
    assert_eq!(original.node_hint, deserialized.node_hint);
    assert_eq!(original.symbol, deserialized.symbol);
    assert_eq!(original.current_price, deserialized.current_price);
    assert_eq!(original.barrier_price, deserialized.barrier_price);
    assert_eq!(original.barrier_type, deserialized.barrier_type);
    assert_eq!(original.transmission_probability, deserialized.transmission_probability);
    assert_eq!(original.expected_profit_bp, deserialized.expected_profit_bp);
}

/// T42.12: Test TunnelingOpportunity equality and cloning
#[test]
fn test_tunneling_opportunity_equality() {
    let scanner = TunnelingScanner::new(42);

    let opportunity1 = scanner.derive_opportunity("BTC/USD", 50_000.0, 51_000.0);
    let opportunity2 = scanner.derive_opportunity("BTC/USD", 50_000.0, 51_000.0);

    // Different instances should not be equal (due to unique IDs)
    assert_ne!(opportunity1, opportunity2);
    assert_ne!(opportunity1.id, opportunity2.id);

    // But other fields should be equal
    assert_eq!(opportunity1.node_hint, opportunity2.node_hint);
    assert_eq!(opportunity1.symbol, opportunity2.symbol);
    assert_eq!(opportunity1.current_price, opportunity2.current_price);
    assert_eq!(opportunity1.barrier_price, opportunity2.barrier_price);
    assert_eq!(opportunity1.barrier_type, opportunity2.barrier_type);
    assert_eq!(opportunity1.transmission_probability, opportunity2.transmission_probability);
    assert_eq!(opportunity1.expected_profit_bp, opportunity2.expected_profit_bp);

    // Test clone equality
    let cloned = opportunity1.clone();
    assert_eq!(opportunity1, cloned);
}

/// T42.13: Test TunnelingOpportunity debug formatting
#[test]
fn test_tunneling_opportunity_debug() {
    let scanner = TunnelingScanner::new(1337);
    let opportunity = scanner.derive_opportunity("ETH/USD", 3_000.0, 2_900.0);

    let debug_str = format!("{:?}", opportunity);
    assert!(debug_str.contains("TunnelingOpportunity"));
    assert!(debug_str.contains("ETH/USD"));
    assert!(debug_str.contains("3000"));
    assert!(debug_str.contains("2900"));
    assert!(debug_str.contains("Support"));
    assert!(debug_str.contains("1337"));
    assert!(debug_str.contains("0.5")); // transmission_probability
}

/// T42.14: Test transmission probability consistency
#[test]
fn test_transmission_probability_consistency() {
    let scanner = TunnelingScanner::new(42);

    // Test multiple opportunities to ensure transmission probability is always 0.5
    for i in 0..100 {
        let opportunity = scanner.derive_opportunity(
            &format!("SYMBOL{}", i),
            1000.0 + i as f64,
            1100.0 + i as f64,
        );

        assert_eq!(opportunity.transmission_probability, 0.5);
    }
}

/// T42.15: Test concurrent tunneling scanner usage
#[test]
fn test_concurrent_tunneling_usage() {
    use std::sync::Arc;
    use std::thread;

    let scanner = Arc::new(TunnelingScanner::new(1337));
    let mut handles = vec![];

    // Spawn multiple threads using the scanner
    for i in 0..10 {
        let scanner_clone = Arc::clone(&scanner);
        let handle = thread::spawn(move || {
            for j in 0..10 {
                let opportunity = scanner_clone.derive_opportunity(
                    &format!("COIN{}/USD", i),
                    1000.0 + (i * 10 + j) as f64,
                    1100.0 + (i * 10 + j) as f64,
                );

                // Verify basic properties
                assert_eq!(opportunity.node_hint, 1337);
                assert_eq!(opportunity.barrier_type, BarrierType::Resistance);
                assert_eq!(opportunity.transmission_probability, 0.5);
                assert_eq!(opportunity.id.class(), aid_class::DOS);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
}

/// T42.16: Property-based test for barrier type determination
#[test]
fn test_barrier_type_determination_properties() {
    let scanner = TunnelingScanner::new(42);

    // Test many random price combinations
    let test_cases = vec![
        (100.0, 150.0), // Resistance
        (100.0, 100.0), // Resistance (equal)
        (100.0, 50.0),  // Support
        (1000.0, 1001.0), // Resistance
        (1000.0, 999.0),  // Support
        (0.1, 0.2),     // Resistance
        (0.2, 0.1),     // Support
        (f64::MIN_POSITIVE, f64::MIN_POSITIVE * 2.0), // Resistance
    ];

    for (current, barrier) in test_cases {
        let opportunity = scanner.derive_opportunity("TEST/USD", current, barrier);

        if barrier >= current {
            assert_eq!(opportunity.barrier_type, BarrierType::Resistance);
        } else {
            assert_eq!(opportunity.barrier_type, BarrierType::Support);
        }

        // Profit calculation should always use absolute value
        let expected_profit = ((barrier - current) / current * 10_000.0_f64).abs() as u32;
        assert_eq!(opportunity.expected_profit_bp, expected_profit);
    }
}

/// T42.17: Test large-scale ID uniqueness
#[test]
fn test_large_scale_id_uniqueness() {
    let scanner = TunnelingScanner::new(42);
    let mut ids = std::collections::HashSet::new();

    // Generate a large number of opportunities to test for ID collisions
    for i in 0..10_000 {
        let opportunity = scanner.derive_opportunity(
            "BTC/USD",
            50_000.0 + (i as f64 * 0.01), // Slightly different prices
            51_000.0 + (i as f64 * 0.01),
        );

        // Should never have duplicate IDs
        assert!(
            ids.insert(opportunity.id),
            "Duplicate ID at iteration {}: {:?}",
            i,
            opportunity.id
        );
    }

    assert_eq!(ids.len(), 10_000);
}
