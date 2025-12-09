use fractal_arbitrage_scanner::HydraCoordinationEngine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 HYDRA Debug Test");
    
    let mut engine = HydraCoordinationEngine::new();
    println!("✅ HYDRA engine created");
    
    let result = engine.add_market_data("BTCUSD", 50000.0, 1000.0, 1640995200000);
    println!("Market data result: {:?}", result);
    
    // Test with minimal data first
    let price_data = vec![50000.0, 50100.0, 49900.0, 50200.0, 49800.0, 50300.0, 49700.0, 50400.0, 49600.0, 50500.0, 49500.0, 50600.0];
    println!("Price data length: {}", price_data.len());
    
    match engine.analyze_unified_arbitrage("BTCUSD", &price_data, 1640995200000) {
        Ok(opportunities) => {
            println!("✅ Analysis successful! Found {} opportunities", opportunities.len());
            for (i, opp) in opportunities.iter().enumerate() {
                println!("  Opportunity {}: {:?} confidence={:.2}", i+1, opp.opportunity_type, opp.confidence);
            }
        }
        Err(e) => {
            println!("❌ Analysis failed: {:?}", e);
            // Let's check if the individual algorithms work
            println!("\n🔍 Testing individual components...");
            
            // The error gives us insight into which algorithm is failing
            return Err(e.into());
        }
    }
    
    Ok(())
}
