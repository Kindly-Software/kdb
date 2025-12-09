//! Fractal Protection System Integration Demo
//!
//! Demonstrates how to use the integrated fractal protection system
//! with adaptive parameters, learned alpha, and proof-of-work validation.

use fractal_arbitrage_scanner::{
    // Core scanner API
    FractalArbitrageScanner, HydraCoordinationEngine,

    // Protection system
    init_basic_protection, init_military_protection, init_performance_protection,
    FractalProtectionManager, ProtectionConfig, ProtectionTier,
    PerformanceRequirements, ProtectionFeatureFlags,

    // Adaptive modules
    MultifractalDFA, LevyFlightDetector,

    // Types
    ArbitrageOpportunity, PerformanceMetrics,
};

use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔒 Fractal Protection System Integration Demo");
    println!("===========================================\n");

    // Demo 1: Basic protection setup
    demo_basic_protection()?;

    // Demo 2: Military-grade protection
    demo_military_protection()?;

    // Demo 3: Performance-optimized protection
    demo_performance_protection()?;

    // Demo 4: Custom protection configuration
    demo_custom_protection()?;

    // Demo 5: Full system integration
    demo_full_system_integration()?;

    println!("\n✅ All protection demos completed successfully!");
    Ok(())
}

fn demo_basic_protection() -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Demo 1: Basic Protection Setup");
    println!("--------------------------------");

    // Initialize basic protection
    let mut protection_manager = init_basic_protection()?;
    let status = protection_manager.get_system_status();

    println!("✓ Protection tier: {:?}", status.protection_tier);
    println!("✓ Features active: {:?}", status.features_active);
    println!("✓ Uptime: {}ms", status.uptime_ms);

    // Wire protection into a simple Hydra engine
    let mut hydra = HydraCoordinationEngine::new();
    protection_manager.wire_hydra_protection(&mut hydra)?;

    println!("✓ Protection wired into Hydra coordination engine");
    println!("✓ Protection enabled: {}\n", hydra.is_protection_enabled());

    Ok(())
}

fn demo_military_protection() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛡️  Demo 2: Military-Grade Protection");
    println!("-----------------------------------");

    let start_time = Instant::now();
    let mut protection_manager = init_military_protection()?;
    let init_time = start_time.elapsed();

    let status = protection_manager.get_system_status();

    println!("✓ Protection tier: {:?}", status.protection_tier);
    println!("✓ Initialization time: {:?}", init_time);
    println!("✓ Features active: {}", status.features_active.len());

    if let Some(ref report) = status.init_report {
        println!("✓ Hardware optimization: {:.1}% improvement",
                 report.performance_improvement_pct);
        println!("✓ Performance tier: {:?}", report.performance_tier);
    }

    // Demonstrate adaptive parameters
    let mut fractal_math = MultifractalDFA::new_adaptive();
    protection_manager.wire_fractal_math_protection(&mut fractal_math)?;

    println!("✓ Adaptive fractal mathematics enabled");

    // Demonstrate alpha learning
    let mut levy_detector = LevyFlightDetector::new_adaptive();
    protection_manager.wire_levy_protection(&mut levy_detector)?;

    println!("✓ Alpha learning enabled in Levy flight detector");

    // Simulate some performance updates
    protection_manager.update_system_performance(150, 0.95, 5 * 1024 * 1024)?;
    protection_manager.update_system_performance(120, 0.97, 4 * 1024 * 1024)?;

    let updated_status = protection_manager.get_system_status();
    println!("✓ Performance stats: {} analyses, {:.2} accuracy",
             updated_status.performance_stats.total_analyses,
             updated_status.performance_stats.current_accuracy);

    println!();
    Ok(())
}

fn demo_performance_protection() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ Demo 3: Performance-Optimized Protection");
    println!("------------------------------------------");

    let start_time = Instant::now();
    let protection_manager = init_performance_protection()?;
    let init_time = start_time.elapsed();

    let status = protection_manager.get_system_status();

    println!("✓ Protection tier: {:?}", status.protection_tier);
    println!("✓ Ultra-fast initialization: {:?}", init_time);
    println!("✓ Features active: {:?}", status.features_active);
    println!("✓ Memory usage: {} bytes", status.performance_stats.memory_usage_bytes);

    // Verify minimal overhead
    if init_time.as_millis() < 50 {
        println!("✓ Initialization under 50ms target");
    }

    println!();
    Ok(())
}

fn demo_custom_protection() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎛️  Demo 4: Custom Protection Configuration");
    println!("------------------------------------------");

    // Create custom configuration
    let config = ProtectionConfig {
        tier: ProtectionTier::Advanced,
        enable_adaptive: true,
        enable_alpha_learning: false, // Disabled for this demo
        performance_requirements: PerformanceRequirements {
            max_init_time_ms: 200,
            max_latency_overhead_pct: 3.0,
            min_accuracy: 0.85,
            max_memory_overhead_mb: 15,
        },
        feature_flags: ProtectionFeatureFlags {
            enable_proof_of_work: true,
            enable_obfuscation: false,
            enable_performance_monitoring: true,
            enable_adaptive_learning: true,
        },
    };

    let mut protection_manager = FractalProtectionManager::new(config);
    let init_results = protection_manager.initialize()?;

    println!("✓ Custom configuration applied");
    println!("✓ Initialization time: {}ms", init_results.init_time_ms);
    println!("✓ Features enabled: {:?}", init_results.features_enabled);

    if !init_results.warnings.is_empty() {
        println!("⚠️  Warnings: {:?}", init_results.warnings);
    }

    println!();
    Ok(())
}

fn demo_full_system_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Demo 5: Full System Integration");
    println!("----------------------------------");

    // Initialize protection
    let mut protection_manager = init_basic_protection()?;

    // Create and wire all components
    let mut hydra = HydraCoordinationEngine::new();
    let mut fractal_math = MultifractalDFA::new();
    let mut levy_detector = LevyFlightDetector::new();

    // Wire protection into all components
    protection_manager.wire_hydra_protection(&mut hydra)?;
    protection_manager.wire_fractal_math_protection(&mut fractal_math)?;
    protection_manager.wire_levy_protection(&mut levy_detector)?;

    println!("✓ All components wired with protection");

    // Simulate some market data analysis
    let market_data = vec![
        100.0, 101.2, 99.8, 102.1, 98.7, 103.4, 97.2, 104.8, 96.1, 105.9,
        95.3, 106.7, 94.8, 107.2, 93.9, 108.1, 92.4, 109.3, 91.7, 110.2
    ];

    println!("📊 Analyzing market data with protected system...");

    let start_analysis = Instant::now();

    // Perform analysis with Hydra (which includes protection)
    let opportunities = hydra.analyze_unified_arbitrage("DEMO", &market_data, 1640995200000)?;

    let analysis_time = start_analysis.elapsed();

    println!("✓ Analysis completed in {:?}", analysis_time);
    println!("✓ Opportunities detected: {}", opportunities.len());

    // Update performance metrics
    let accuracy = if opportunities.is_empty() { 0.8 } else { 0.92 };
    protection_manager.update_system_performance(
        analysis_time.as_micros() as u64,
        accuracy,
        1024 * 1024 // 1MB memory usage estimate
    )?;

    // Get final system status
    let final_status = protection_manager.get_system_status();
    println!("✓ Final system status: {} analyses completed",
             final_status.performance_stats.total_analyses);

    // Demonstrate adaptive learning
    let sample_jumps = vec![0.01, 0.005, 0.02, 0.003, 0.015];
    levy_detector.learn_alpha(&sample_jumps, accuracy)?;

    let alpha_stats = levy_detector.get_alpha_stats();
    println!("✓ Learned alpha: {:.3} (samples: {})",
             alpha_stats.current_alpha, alpha_stats.learning_samples);

    println!();
    Ok(())
}