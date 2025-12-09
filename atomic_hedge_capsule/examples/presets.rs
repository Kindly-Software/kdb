//! # AtomicHedgeCapsule Preset Configurations Example
//!
//! This example demonstrates how to use preset configurations for common trading scenarios.
//! Each preset is optimized for specific use cases with documented performance characteristics.
//!
//! Run with: `cargo run --example presets --features "builder,presets"`

use atomic_hedge_capsule::{AtomicHedgeCapsule, AtomicHedgeCapsulePresets, PresetConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 AtomicHedgeCapsule Preset Configurations Demo\n");

    // 1. High Frequency Trading Preset
    println!("1. High Frequency Trading Preset");
    println!("   Target: < 50ns latency for ultra-fast execution");

    let hft_capsule = AtomicHedgeCapsule::with_hft_preset("BTCUSD", "NDAX", 0.1, 50000.0, 52000.0)?;

    hft_capsule.submit_order()?;
    let hft_status = hft_capsule.status();
    println!("   Status: {}", hft_status);
    println!("   Safe: {}\n", hft_status.is_safe());

    // 2. Risk Management Preset
    println!("2. Risk Management Preset");
    println!("   Target: Maximum safety with comprehensive validation");

    let risk_capsule = AtomicHedgeCapsule::with_risk_preset("ETHUSD", "NDAX", 0.5, 3000.0, 3500.0)?;

    risk_capsule.submit_order()?;
    let risk_status = risk_capsule.status();
    println!("   Status: {}", risk_status);
    println!("   Safe: {}\n", risk_status.is_safe());

    // 3. Arbitrage Preset
    println!("3. Arbitrage Preset");
    println!("   Target: Cross-exchange optimization");

    let arb_capsule =
        AtomicHedgeCapsule::with_arbitrage_preset("BTCUSD", "Binance", 1.0, 49500.0, 50500.0)?;

    arb_capsule.submit_order()?;
    let arb_status = arb_capsule.status();
    println!("   Status: {}", arb_status);
    println!("   Safe: {}\n", arb_status.is_safe());

    // 4. Development Preset
    println!("4. Development Preset");
    println!("   Target: Debug-friendly with detailed logging");

    let dev_capsule =
        AtomicHedgeCapsule::with_development_preset("TESTUSD", "NDAX", 0.01, 1000.0, 1100.0)?;

    dev_capsule.submit_order()?;
    let dev_status = dev_capsule.status();
    println!("   Status: {}", dev_status);
    println!("   Safe: {}\n", dev_status.is_safe());

    // 5. Production Preset
    println!("5. Production Preset");
    println!("   Target: Battle-tested configuration for production");

    let prod_capsule =
        AtomicHedgeCapsule::with_production_preset("BTCUSD", "NDAX", 2.0, 48000.0, 52000.0)?;

    prod_capsule.submit_order()?;
    let prod_status = prod_capsule.status();
    println!("   Status: {}", prod_status);
    println!("   Safe: {}\n", prod_status.is_safe());

    // Configuration Analysis
    println!("📊 Preset Configuration Analysis\n");

    let configs = [
        ("HFT", PresetConfig::high_frequency_trading()),
        ("Risk Management", PresetConfig::risk_management()),
        ("Arbitrage", PresetConfig::arbitrage()),
        ("Development", PresetConfig::development()),
        ("Production", PresetConfig::production()),
    ];

    for (name, config) in &configs {
        println!("{}: {}", name, config.performance_description());
        println!(
            "   Performance: {:.2}x baseline",
            config.estimated_performance_multiplier()
        );
        println!("   Risk: {}", config.risk_profile());
        println!();
    }

    // Builder Pattern Example
    println!("🔧 Using Builder Pattern with Presets\n");

    #[cfg(feature = "builder")]
    {
        let custom_capsule = AtomicHedgeCapsule::hft_preset()
            .with_emergency_threshold(0.002) // Custom threshold
            .with_timeout_ms(75) // Custom timeout
            .with_entry_order("Kraken", "XBTUSD", "Buy", 0.5)
            .with_bracket_order(49000.0, 51000.0)
            .build()?;

        custom_capsule.submit_order()?;
        let custom_status = custom_capsule.status();
        println!("Custom HFT Configuration:");
        println!("   Status: {}", custom_status);
        println!("   Safe: {}", custom_status.is_safe());
    }

    // Performance Demonstration
    println!("\n⚡ Performance Demonstration\n");

    // Simulate execution for each preset
    let test_amount = 0.5;
    let executions = [
        ("HFT", &hft_capsule),
        ("Risk Management", &risk_capsule),
        ("Arbitrage", &arb_capsule),
        ("Development", &dev_capsule),
        ("Production", &prod_capsule),
    ];

    for (name, capsule) in &executions {
        if capsule.is_ready_to_hedge() {
            let result = capsule.execute_hedge(test_amount)?;
            println!(
                "{}: {} (filled: {:.3})",
                name,
                if result.success {
                    "✅ Success"
                } else {
                    "❌ Failed"
                },
                result.entry_filled
            );
        } else {
            println!("{}: ⚠️  Not ready for execution", name);
        }
    }

    println!("\n🎯 Summary");
    println!("All preset configurations successfully created and executed!");
    println!("Each preset is optimized for its specific trading scenario:");
    println!("- HFT: Ultra-low latency (< 50ns target)");
    println!("- Risk Management: Maximum safety and validation");
    println!("- Arbitrage: Cross-exchange coordination optimization");
    println!("- Development: Debug-friendly with comprehensive logging");
    println!("- Production: Battle-tested balance of performance and reliability");

    Ok(())
}
