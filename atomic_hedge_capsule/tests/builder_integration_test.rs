//! Builder Pattern Integration Tests
//!
//! UCE-32 Q30: Empirical validation of builder pattern performance and correctness

use atomic_hedge_capsule::{AtomicHedgeCapsule, HedgeCapsuleBuilder, HedgeError};

#[test]
fn test_builder_vs_direct_construction_performance() {
    use std::time::Instant;

    const ITERATIONS: usize = 10_000;

    // Test direct construction performance
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let capsule =
            AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();
        // Prevent optimization
        std::hint::black_box(capsule);
    }
    let direct_duration = start.elapsed();

    // Test builder construction performance
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let capsule = AtomicHedgeCapsule::builder()
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0)
            .build()
            .unwrap();
        // Prevent optimization
        std::hint::black_box(capsule);
    }
    let builder_duration = start.elapsed();

    // UCE-32 Q30: Empirical validation - builder should be within 5% of direct construction
    let overhead_ratio = builder_duration.as_nanos() as f64 / direct_duration.as_nanos() as f64;
    println!("Direct construction: {:?}", direct_duration);
    println!("Builder construction: {:?}", builder_duration);
    println!("Overhead ratio: {:.3}x", overhead_ratio);

    assert!(
        overhead_ratio < 1.05,
        "Builder overhead too high: {:.3}x (should be < 1.05x)",
        overhead_ratio
    );
}

#[test]
fn test_preset_configurations_integration() {
    // Test high-frequency trading preset
    let hft_capsule = AtomicHedgeCapsule::high_frequency_trading()
        .with_entry_order("Binance", "BTCUSDT", "Buy", 0.1)
        .with_bracket_order(50000.0, 52000.0)
        .build()
        .unwrap();

    assert!(hft_capsule.is_active());
    assert!(!hft_capsule.is_emergency_stopped());

    // Test conservative trading preset
    let conservative_capsule = AtomicHedgeCapsule::conservative_trading()
        .with_entry_order("Coinbase", "BTC-USD", "Buy", 0.5)
        .with_bracket_order(45000.0, 55000.0)
        .build()
        .unwrap();

    assert!(conservative_capsule.is_active());
    assert!(!conservative_capsule.is_emergency_stopped());

    // Test market making preset
    let mm_capsule = AtomicHedgeCapsule::market_making()
        .with_entry_order("Kraken", "XBTUSD", "Buy", 2.0)
        .with_bracket_order(48000.0, 52000.0)
        .build()
        .unwrap();

    assert!(mm_capsule.is_active());
    assert!(!mm_capsule.is_emergency_stopped());
}

#[test]
fn test_builder_error_handling() {
    // Test missing required fields - this should be a compile-time error
    // due to type-state pattern, so we test quick_build with invalid params instead
    let result = HedgeCapsuleBuilder::quick_build(
        "", // Invalid: empty exchange
        "BTCUSD", 1.0, 45000.0, 55000.0,
    );

    assert!(result.is_ok()); // Empty exchange gets default value

    // Test invalid emergency threshold
    let result = AtomicHedgeCapsule::builder()
        .with_emergency_threshold(1.5) // Invalid: > 1.0
        .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
        .with_bracket_order(45000.0, 55000.0)
        .build();

    assert!(result.is_err());
    if let Err(HedgeError::ValidationFailed { field, .. }) = result {
        assert_eq!(field, "emergency_threshold");
    } else {
        panic!("Expected ValidationFailed error");
    }
}

#[test]
fn test_builder_state_transitions() {
    // Test proper type-state transitions
    let builder = AtomicHedgeCapsule::builder()
        .with_emergency_threshold(0.02)
        .with_cache_optimization();

    // After adding entry order, should be in WithEntry state
    let builder_with_entry = builder.with_entry_order("NDAX", "BTCUSD", "Buy", 1.0);

    // After adding bracket order, should be in WithBracket state and ready to build
    let capsule = builder_with_entry
        .with_bracket_order(45000.0, 55000.0)
        .build()
        .unwrap();

    assert!(capsule.is_active());
}

#[test]
fn test_quick_build_convenience() {
    // Test quick build method
    let capsule = HedgeCapsuleBuilder::quick_build("NDAX", "ETHUSD", 2.0, 3000.0, 4000.0).unwrap();

    assert!(capsule.is_active());
    assert!(!capsule.is_emergency_stopped());

    // Test minimal build
    let capsule = HedgeCapsuleBuilder::minimal_build().unwrap();
    assert!(capsule.is_active());
}

#[test]
fn test_builder_configuration_options() {
    let capsule = AtomicHedgeCapsule::builder()
        .with_emergency_threshold(0.01)
        .with_cache_optimization()
        .with_max_position_size(500.0)
        .with_timeout_ms(1000)
        .with_exchange("Binance")
        .with_symbol("ETHUSDT")
        .with_entry_order("Binance", "ETHUSDT", "Buy", 1.0)
        .with_bracket_order(3000.0, 4000.0)
        .build()
        .unwrap();

    assert!(capsule.is_active());
    assert!(!capsule.is_emergency_stopped());

    // Test state operations
    let status = capsule.status();
    assert!(status.is_safe());
    assert!(!status.needs_attention());
}

#[test]
fn test_builder_memory_safety() {
    use std::thread;

    // Test builder in multi-threaded environment
    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                let capsule = AtomicHedgeCapsule::builder()
                    .with_emergency_threshold(0.02)
                    .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
                    .with_bracket_order(45000.0, 55000.0)
                    .build()
                    .unwrap();

                // Test thread safety
                for _ in 0..100 {
                    let _ = capsule.is_active();
                    let _ = capsule.get_hedge_state();
                }

                (i, capsule.is_active())
            })
        })
        .collect();

    // All threads should complete successfully
    for handle in handles {
        let (thread_id, is_active) = handle.join().unwrap();
        assert!(is_active, "Thread {} capsule should be active", thread_id);
    }
}

#[test]
fn test_builder_validation_edge_cases() {
    // Test NaN values
    let result = AtomicHedgeCapsule::builder()
        .with_emergency_threshold(f64::NAN)
        .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
        .with_bracket_order(45000.0, 55000.0)
        .build();
    assert!(result.is_err());

    // Test infinity values
    let result = AtomicHedgeCapsule::builder()
        .with_entry_order("NDAX", "BTCUSD", "Buy", f64::INFINITY)
        .with_bracket_order(45000.0, 55000.0)
        .build();
    assert!(result.is_err());

    // Test negative values
    let result = AtomicHedgeCapsule::builder()
        .with_entry_order("NDAX", "BTCUSD", "Buy", -1.0)
        .with_bracket_order(45000.0, 55000.0)
        .build();
    assert!(result.is_err());

    // Test zero timeout
    let result = AtomicHedgeCapsule::builder()
        .with_timeout_ms(0)
        .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
        .with_bracket_order(45000.0, 55000.0)
        .build();
    assert!(result.is_err());
}

#[test]
fn test_builder_realistic_scenarios() {
    // Scenario 1: Day trading setup
    let day_trading = AtomicHedgeCapsule::builder()
        .with_emergency_threshold(0.02) // 2% emergency threshold
        .with_max_position_size(100.0) // Conservative size
        .with_timeout_ms(5000) // 5 second timeout
        .with_entry_order("NDAX", "BTCUSD", "Buy", 0.5)
        .with_bracket_order(49000.0, 51000.0) // Tight range
        .build()
        .unwrap();

    assert!(day_trading.is_active());

    // Scenario 2: Swing trading setup
    let swing_trading = AtomicHedgeCapsule::conservative_trading()
        .with_max_position_size(50.0) // Smaller position
        .with_timeout_ms(30000) // Longer timeout
        .with_entry_order("Coinbase", "ETH-USD", "Buy", 1.0)
        .with_bracket_order(2800.0, 3200.0) // Wider range
        .build()
        .unwrap();

    assert!(swing_trading.is_active());

    // Scenario 3: Scalping setup
    let scalping = AtomicHedgeCapsule::high_frequency_trading()
        .with_emergency_threshold(0.005) // Tight threshold
        .with_timeout_ms(100) // Ultra-fast
        .with_entry_order("Binance", "BTCUSDT", "Buy", 0.1)
        .with_bracket_order(50100.0, 50200.0) // Very tight range
        .build()
        .unwrap();

    assert!(scalping.is_active());
}

#[cfg(feature = "nightly")]
#[test]
fn test_nightly_builder_features() {
    // Test algorithmic trading preset
    let algo_capsule = AtomicHedgeCapsule::builder()
        .algorithmic_trading()
        .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
        .with_bracket_order(45000.0, 55000.0)
        .build()
        .unwrap();

    assert!(algo_capsule.is_active());

    // Test quantitative research preset
    let research_capsule = AtomicHedgeCapsule::builder()
        .quantitative_research()
        .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
        .with_bracket_order(45000.0, 55000.0)
        .build()
        .unwrap();

    assert!(research_capsule.is_active());
}

#[cfg(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic"))]
#[test]
fn test_const_builder_functionality() {
    // Test const builder creation
    const BUILDER: HedgeCapsuleBuilder<_> = HedgeCapsuleBuilder::const_new();

    let capsule = BUILDER
        .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
        .with_bracket_order(45000.0, 55000.0)
        .build()
        .unwrap();

    assert!(capsule.is_active());
}

#[test]
fn test_builder_api_consistency() {
    // Test that all builder methods return the expected types and can be chained
    let _ = AtomicHedgeCapsule::builder()
        .with_emergency_threshold(0.02)
        .with_cache_optimization()
        .without_cache_optimization()
        .with_cache_optimization()
        .with_max_position_size(1000.0)
        .with_timeout_ms(5000)
        .with_exchange("NDAX")
        .with_symbol("BTCUSD")
        .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
        .with_bracket_order(45000.0, 55000.0)
        .build()
        .unwrap();

    // Test preset chaining
    let _ = AtomicHedgeCapsule::high_frequency_trading()
        .with_max_position_size(200.0)
        .with_entry_order("Binance", "ETHUSDT", "Buy", 2.0)
        .with_bracket_order(3000.0, 4000.0)
        .build()
        .unwrap();
}
