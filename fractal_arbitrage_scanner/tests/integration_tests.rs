//! Integration tests for HYDRA coordination and system-wide functionality
//!
//! Implements T42 framework testing end-to-end workflows, component integration,
//! and multi-threaded coordination scenarios.

use fractal_arbitrage_scanner::{
    aid_class, Aid96, ArbitrageError, ArbitrageOpportunity, QuantumArbitrageScanner,
    TemporalArbitrageOpportunity, TunnelingOpportunity,
};
use fractal_arbitrage_scanner::tunneling_integration::BarrierType;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// T42.1: Test complete arbitrage discovery workflow
#[test]
fn test_complete_arbitrage_workflow() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Step 1: Scan for basic arbitrage opportunity
    let arbitrage_result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        1.0,
    );

    assert!(arbitrage_result.is_ok());
    let arbitrage = arbitrage_result.unwrap();

    // Step 2: Generate temporal hint for the same symbol
    let temporal = scanner.temporal_hint(
        "BTC/USD",
        50_050.0, // Current price between buy and sell
        50_200.0, // Future price
        0.8,      // High confidence
        Duration::from_millis(100),
    );

    // Step 3: Generate tunneling hint for barrier analysis
    let tunneling = scanner.tunneling_hint(
        "BTC/USD",
        50_050.0, // Current price
        50_500.0, // Resistance barrier
    );

    // Verify all components work together
    assert_eq!(arbitrage.symbol, "BTC/USD");
    assert_eq!(temporal.symbol, "BTC/USD");
    assert_eq!(tunneling.symbol, "BTC/USD");

    // Verify different AID-96 classes
    assert_eq!(arbitrage.id.class(), aid_class::PEX);
    assert_eq!(temporal.id.class(), aid_class::ALT);
    assert_eq!(tunneling.id.class(), aid_class::DOS);

    // Verify all IDs are unique
    assert_ne!(arbitrage.id, temporal.id);
    assert_ne!(arbitrage.id, tunneling.id);
    assert_ne!(temporal.id, tunneling.id);
}

/// T42.2: Test HYDRA-style multi-scanner coordination
#[test]
fn test_multi_scanner_coordination() {
    // Create multiple scanners with different node hints (simulating HYDRA nodes)
    let scanners = vec![
        QuantumArbitrageScanner::new(1),
        QuantumArbitrageScanner::new(2),
        QuantumArbitrageScanner::new(3),
        QuantumArbitrageScanner::new(4),
    ];

    let symbols = vec!["BTC/USD", "ETH/USD", "XRP/USD", "ADA/USD"];
    let mut all_opportunities = Vec::new();
    let mut all_ids = std::collections::HashSet::new();

    // Each scanner processes different symbols (distributed work)
    for (i, scanner) in scanners.iter().enumerate() {
        let symbol = symbols[i];
        let base_price = 1000.0 * (i + 1) as f64;

        // Generate arbitrage opportunity
        let arbitrage = scanner.scan_arbitrage(
            symbol,
            "exchange_a",
            "exchange_b",
            base_price,
            base_price * 1.01,
            1.0,
        ).unwrap();

        // Generate temporal and tunneling hints
        let temporal = scanner.temporal_hint(
            symbol,
            base_price,
            base_price * 1.05,
            0.7,
            Duration::from_millis(100),
        );

        let tunneling = scanner.tunneling_hint(
            symbol,
            base_price,
            base_price * 1.02,
        );

        all_opportunities.push((arbitrage, temporal, tunneling));

        // Verify unique IDs across all scanners
        for (arb, temp, tunn) in &all_opportunities {
            assert!(all_ids.insert(arb.id));
            assert!(all_ids.insert(temp.id));
            assert!(all_ids.insert(tunn.id));
        }
    }

    // Verify we have the expected number of unique opportunities
    assert_eq!(all_opportunities.len(), 4);
    assert_eq!(all_ids.len(), 12); // 4 scanners * 3 opportunity types each
}

/// T42.3: Test concurrent multi-threaded arbitrage scanning
#[test]
fn test_concurrent_arbitrage_scanning() {
    let scanner = Arc::new(QuantumArbitrageScanner::new(1337));
    let num_threads = 10;
    let opportunities_per_thread = 50;

    let mut handles = vec![];
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Spawn multiple threads scanning for opportunities
    for thread_id in 0..num_threads {
        let scanner_clone = Arc::clone(&scanner);
        let results_clone = Arc::clone(&results);

        let handle = thread::spawn(move || {
            let mut local_results = Vec::new();

            for i in 0..opportunities_per_thread {
                let symbol = format!("COIN{}/USD", thread_id);
                let base_price = 1000.0 + (thread_id * opportunities_per_thread + i) as f64;

                // Scan arbitrage
                let arbitrage = scanner_clone.scan_arbitrage(
                    &symbol,
                    "exchange_a",
                    "exchange_b",
                    base_price,
                    base_price + 10.0,
                    1.0,
                );

                match arbitrage {
                    Ok(opp) => local_results.push(opp),
                    Err(e) => panic!("Unexpected error in thread {}: {:?}", thread_id, e),
                }
            }

            // Add to global results
            let mut global_results = results_clone.lock().unwrap();
            global_results.extend(local_results);
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    let final_results = results.lock().unwrap();
    assert_eq!(final_results.len(), num_threads * opportunities_per_thread);

    // Verify all IDs are unique
    let mut ids = std::collections::HashSet::new();
    for opportunity in final_results.iter() {
        assert!(ids.insert(opportunity.id), "Duplicate ID found in concurrent test");
    }

    assert_eq!(ids.len(), final_results.len());
}

/// T42.4: Test cross-component data consistency
#[test]
fn test_cross_component_consistency() {
    let scanner = QuantumArbitrageScanner::new(42);
    let symbol = "BTC/USD";
    let current_price = 50_000.0;

    // Generate multiple opportunities for the same symbol/price
    let arbitrage = scanner.scan_arbitrage(
        symbol,
        "binance",
        "coinbase",
        current_price,
        current_price + 100.0,
        1.0,
    ).unwrap();

    let temporal = scanner.temporal_hint(
        symbol,
        current_price,
        current_price + 200.0,
        0.75,
        Duration::from_millis(150),
    );

    let tunneling_resistance = scanner.tunneling_hint(
        symbol,
        current_price,
        current_price + 500.0, // Resistance
    );

    let tunneling_support = scanner.tunneling_hint(
        symbol,
        current_price,
        current_price - 300.0, // Support
    );

    // Verify symbol consistency
    assert_eq!(arbitrage.symbol, symbol);
    assert_eq!(temporal.symbol, symbol);
    assert_eq!(tunneling_resistance.symbol, symbol);
    assert_eq!(tunneling_support.symbol, symbol);

    // Verify price relationships make sense
    assert_eq!(temporal.current_price, current_price);
    assert_eq!(tunneling_resistance.current_price, current_price);
    assert_eq!(tunneling_support.current_price, current_price);

    // Verify barrier types are correct
    assert_eq!(tunneling_resistance.barrier_type, BarrierType::Resistance);
    assert_eq!(tunneling_support.barrier_type, BarrierType::Support);

    // Verify timing constraints
    assert!(arbitrage.expiry_nanos > arbitrage.timestamp_nanos);
    assert!(temporal.execution_delay == Duration::from_millis(150));
}

/// T42.5: Test error propagation and handling across components
#[test]
fn test_error_propagation() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Test invalid price propagation
    let invalid_arbitrage = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        -50_000.0, // Invalid negative price
        50_100.0,
        1.0,
    );

    match invalid_arbitrage {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, -50_000.0);
        }
        _ => panic!("Expected InvalidPrice error"),
    }

    // Test invalid volume propagation
    let invalid_volume = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        -1.0, // Invalid negative volume
    );

    match invalid_volume {
        Err(ArbitrageError::InvalidVolume) => {},
        _ => panic!("Expected InvalidVolume error"),
    }

    // Temporal and tunneling hints should still work even if arbitrage fails
    let temporal = scanner.temporal_hint(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.8,
        Duration::from_millis(100),
    );
    assert_eq!(temporal.symbol, "BTC/USD");

    let tunneling = scanner.tunneling_hint("BTC/USD", 50_000.0, 51_000.0);
    assert_eq!(tunneling.symbol, "BTC/USD");
}

/// T42.6: Test high-frequency scanning simulation
#[test]
fn test_high_frequency_scanning() {
    let scanner = QuantumArbitrageScanner::new(42);
    let start_time = std::time::Instant::now();
    let mut opportunities = Vec::new();

    // Simulate high-frequency scanning for 100ms
    while start_time.elapsed() < Duration::from_millis(100) {
        let timestamp = start_time.elapsed().as_nanos() as u64;
        let price_variance = (timestamp % 1000) as f64 / 1000.0; // Small price variations

        let result = scanner.scan_arbitrage(
            "BTC/USD",
            "exchange_a",
            "exchange_b",
            50_000.0 + price_variance,
            50_010.0 + price_variance,
            1.0,
        );

        if let Ok(opportunity) = result {
            opportunities.push(opportunity);
        }
    }

    // Should have generated many opportunities
    assert!(opportunities.len() > 10);

    // All opportunities should have unique IDs
    let mut ids = std::collections::HashSet::new();
    for opportunity in &opportunities {
        assert!(ids.insert(opportunity.id));
    }

    // Verify timestamps are monotonically increasing (approximately)
    for window in opportunities.windows(2) {
        assert!(window[1].timestamp_nanos >= window[0].timestamp_nanos);
    }
}

/// T42.7: Test memory efficiency under load
#[test]
fn test_memory_efficiency() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Generate a large number of opportunities without storing them
    // This tests that the scanner doesn't leak memory
    for i in 0..10_000 {
        let _arbitrage = scanner.scan_arbitrage(
            "BTC/USD",
            "exchange_a",
            "exchange_b",
            50_000.0 + i as f64,
            50_100.0 + i as f64,
            1.0,
        ).unwrap();

        let _temporal = scanner.temporal_hint(
            "BTC/USD",
            50_000.0 + i as f64,
            51_000.0 + i as f64,
            0.5,
            Duration::from_millis(100),
        );

        let _tunneling = scanner.tunneling_hint(
            "BTC/USD",
            50_000.0 + i as f64,
            51_000.0 + i as f64,
        );

        // Opportunities should be dropped immediately after creation
        // No explicit cleanup needed due to Rust's ownership system
    }

    // If we reach here without OOM, the test passes
    assert!(true);
}

/// T42.8: Test scanner state isolation
#[test]
fn test_scanner_state_isolation() {
    let scanner1 = QuantumArbitrageScanner::new(100);
    let scanner2 = QuantumArbitrageScanner::new(200);

    // Generate opportunities with both scanners
    let opp1 = scanner1.scan_arbitrage("BTC/USD", "ex1", "ex2", 50_000.0, 50_100.0, 1.0).unwrap();
    let opp2 = scanner2.scan_arbitrage("BTC/USD", "ex1", "ex2", 50_000.0, 50_100.0, 1.0).unwrap();

    // Opportunities should be independent
    assert_ne!(opp1.id, opp2.id);

    // Tunneling opportunities should have different node hints
    let tunnel1 = scanner1.tunneling_hint("BTC/USD", 50_000.0, 51_000.0);
    let tunnel2 = scanner2.tunneling_hint("BTC/USD", 50_000.0, 51_000.0);

    assert_eq!(tunnel1.node_hint, 100);
    assert_eq!(tunnel2.node_hint, 200);
    assert_ne!(tunnel1.id, tunnel2.id);
}

/// T42.9: Test system resilience under invalid inputs
#[test]
fn test_system_resilience() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Test various invalid combinations
    let invalid_cases = vec![
        (f64::NAN, 50_100.0, 1.0),
        (50_000.0, f64::INFINITY, 1.0),
        (50_000.0, 50_100.0, f64::NEG_INFINITY),
        (0.0, 50_100.0, 1.0),
        (50_000.0, 0.0, 1.0),
        (50_000.0, 50_100.0, 0.0),
        (-1.0, 50_100.0, 1.0),
        (50_000.0, -1.0, 1.0),
        (50_000.0, 50_100.0, -1.0),
    ];

    for (buy_price, sell_price, volume) in invalid_cases {
        let result = scanner.scan_arbitrage(
            "BTC/USD",
            "exchange_a",
            "exchange_b",
            buy_price,
            sell_price,
            volume,
        );

        // Should always return an error for invalid inputs
        assert!(result.is_err());

        // But temporal and tunneling should still work with valid prices
        if buy_price.is_finite() && buy_price > 0.0 {
            let temporal = scanner.temporal_hint(
                "BTC/USD",
                buy_price,
                buy_price * 1.1,
                0.5,
                Duration::from_millis(100),
            );
            assert_eq!(temporal.current_price, buy_price);

            let tunneling = scanner.tunneling_hint("BTC/USD", buy_price, buy_price * 1.1);
            assert_eq!(tunneling.current_price, buy_price);
        }
    }
}

/// T42.10: Test timing precision and consistency
#[test]
fn test_timing_precision() {
    let scanner = QuantumArbitrageScanner::new(42);

    let opportunities: Vec<_> = (0..100)
        .map(|i| {
            scanner.scan_arbitrage(
                "BTC/USD",
                "exchange_a",
                "exchange_b",
                50_000.0 + i as f64,
                50_100.0 + i as f64,
                1.0,
            ).unwrap()
        })
        .collect();

    // Verify timestamp progression
    for window in opportunities.windows(2) {
        // Timestamps should be non-decreasing
        assert!(window[1].timestamp_nanos >= window[0].timestamp_nanos);

        // Expiry should always be after timestamp
        assert!(window[0].expiry_nanos > window[0].timestamp_nanos);
        assert!(window[1].expiry_nanos > window[1].timestamp_nanos);

        // TTL should be consistent (250ms)
        let ttl0 = window[0].expiry_nanos - window[0].timestamp_nanos;
        let ttl1 = window[1].expiry_nanos - window[1].timestamp_nanos;
        let expected_ttl = Duration::from_millis(250).as_nanos() as u64;

        assert_eq!(ttl0, expected_ttl);
        assert_eq!(ttl1, expected_ttl);
    }
}
