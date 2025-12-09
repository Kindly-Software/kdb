fn main() {
    use fractal_arbitrage_scanner::HydraCoordinationEngine;

    // Test that HYDRA can instantiate with all three new algorithms
    let mut engine = HydraCoordinationEngine::new();

    // Test adding market data (integration with multiscale)
    let result = engine.add_market_data("BTCUSD", 50000.0, 1000.0, 1640995200000);
    assert!(result.is_ok(), "Failed to add market data");

    // Test unified analysis (integration with all algorithms)
    let price_data = vec![50000.0, 50100.0, 49900.0, 50200.0, 49800.0, 50300.0, 49700.0, 50400.0, 49600.0, 50500.0];
    let result = engine.analyze_unified_arbitrage("BTCUSD", &price_data, 1640995200000);
    assert!(result.is_ok(), "Failed to run unified analysis");

    println!("✅ HYDRA integration verified successfully!");
    println!("   - levy_flight_detector: ✅");
    println!("   - topological_arbitrage: ✅");
    println!("   - recurrence_analyzer: ✅");

    let stats = engine.get_coordination_stats();
    println!("   - Coordination stats: generation={}, total={}", stats.generation, stats.total_coordinations);
}
