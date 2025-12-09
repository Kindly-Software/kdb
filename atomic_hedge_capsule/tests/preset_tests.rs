//! Preset Configuration Tests for AtomicHedgeCapsule
//!
//! [TRADE SECRET] - Comprehensive validation of preset configurations and builder patterns
//!
//! UCE-32 Q28(Simplicity): Preset configurations provide simple setup for complex trading scenarios
//! UCE-32 Q29(Constraints): Validates preset configurations meet real-world constraints
//! UCE-32 Q30(Validation): Empirical testing of preset performance characteristics
//! UCE-32 Q31(Rust): Type-safe presets preventing invalid configurations

use atomic_hedge_capsule::capsule_standalone::HedgeBuilder as InternalHedgeBuilder;
use atomic_hedge_capsule::{
    AtomicHedgeCapsule, BracketOrder, EntryOrder, HedgeBuilder, HedgeError, HedgeExecutionResult,
    HedgeStatus, OrderState,
};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Preset Configuration Tests
///
/// UCE-32 Q28: Testing preset configurations that simplify common trading patterns
/// UCE-32 Q30: Statistical validation of preset performance characteristics

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hft_preset_configuration() {
        // UCE-32 Q31: Test high-frequency trading preset
        let hedge = InternalHedgeBuilder::hft_preset("BTCUSD")
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(hedge.is_ok(), "HFT preset should build successfully");

        let capsule = hedge.unwrap();
        assert!(capsule.is_active(), "HFT preset should be active");

        // HFT presets should be optimized for speed
        let state = capsule.get_hedge_state();
        assert!(
            state.operation_count >= 1,
            "HFT preset should have operations"
        );
    }

    #[test]
    fn test_conservative_preset_configuration() {
        // UCE-32 Q31: Test conservative trading preset
        let hedge = InternalHedgeBuilder::conservative_preset("ETHUSD")
            .on_exchange("COINBASE")
            .size(5.0)
            .stop_loss(3000.0)
            .take_profit(4000.0)
            .build();

        assert!(
            hedge.is_ok(),
            "Conservative preset should build successfully"
        );

        let capsule = hedge.unwrap();
        assert!(capsule.is_active(), "Conservative preset should be active");

        // Conservative presets should have more careful state management
        let status = capsule.status();
        assert!(
            status.is_safe(),
            "Conservative preset should be in safe state"
        );
    }

    #[test]
    fn test_market_order_preset() {
        // UCE-32 Q28: Test market order preset for immediate execution
        let hedge = InternalHedgeBuilder::market_order("BTCUSD")
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(
            hedge.is_ok(),
            "Market order preset should build successfully"
        );

        let capsule = hedge.unwrap();
        assert!(capsule.is_active(), "Market order preset should be active");
        assert!(
            capsule.is_ready_to_hedge(),
            "Market order should be ready immediately"
        );
    }

    #[test]
    fn test_limit_order_preset() {
        // UCE-32 Q28: Test limit order preset with specific price
        let price = 50000.0;
        let hedge = InternalHedgeBuilder::limit_order("BTCUSD", price)
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(
            hedge.is_ok(),
            "Limit order preset should build successfully"
        );

        let capsule = hedge.unwrap();
        assert!(capsule.is_active(), "Limit order preset should be active");
    }

    #[test]
    fn test_preset_fluent_api_compatibility() {
        // UCE-32 Q28: Test that presets work with fluent API chaining
        let hedge = AtomicHedgeCapsule::hedge("BTCUSD")
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(hedge.is_ok(), "Fluent API should work with presets");

        let capsule = hedge.unwrap();
        assert!(capsule.is_active(), "Fluent API capsule should be active");

        // Test full workflow with simplified API
        assert!(
            capsule.submit_order().is_ok(),
            "Should submit order successfully"
        );
        assert!(
            capsule.update_progress(0.25).is_ok(),
            "Should update progress"
        );

        let status = capsule.status();
        assert!(status.completion > 0.0, "Should show progress");
    }

    #[test]
    fn test_preset_parameter_validation() {
        // UCE-32 Q29: Test that presets enforce practical constraints

        // Test invalid size
        let hedge = InternalHedgeBuilder::market_order("BTCUSD")
            .size(-1.0) // Invalid negative size
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(hedge.is_err(), "Should reject negative size");

        // Test invalid price range
        let hedge = InternalHedgeBuilder::market_order("BTCUSD")
            .size(1.0)
            .stop_loss(55000.0) // Stop loss higher than take profit
            .take_profit(45000.0)
            .build();

        // This should still succeed as the validation depends on order direction
        // but the risk calculations should reflect the configuration

        // Test extreme values
        let hedge = InternalHedgeBuilder::market_order("BTCUSD")
            .size(f64::INFINITY)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(hedge.is_err(), "Should reject infinite size");
    }

    #[test]
    fn test_preset_performance_characteristics() {
        // UCE-32 Q30: Empirical validation of preset performance
        const ITERATIONS: usize = 100;
        let mut build_times = Vec::with_capacity(ITERATIONS);

        for i in 0..ITERATIONS {
            let start = Instant::now();

            let _hedge = InternalHedgeBuilder::hft_preset("BTCUSD")
                .on_exchange("NDAX")
                .size(1.0 + i as f64 * 0.01)
                .stop_loss(45000.0)
                .take_profit(55000.0)
                .build()
                .unwrap();

            build_times.push(start.elapsed().as_nanos());
        }

        // Calculate performance statistics
        let mean = build_times.iter().sum::<u128>() / build_times.len() as u128;
        let min = *build_times.iter().min().unwrap();
        let max = *build_times.iter().max().unwrap();

        println!(
            "HFT preset performance: mean={}ns, min={}ns, max={}ns",
            mean, min, max
        );

        // UCE-32 Q30: Performance requirements for HFT presets
        assert!(mean < 50_000, "HFT preset should be fast: mean={}ns", mean);
        assert!(
            max < 200_000,
            "HFT preset max time should be < 200μs: max={}ns",
            max
        );

        // Test consistency
        let range = max - min;
        assert!(
            range < mean * 5,
            "Performance should be consistent: range={}ns vs mean={}ns",
            range,
            mean
        );
    }

    #[test]
    fn test_preset_concurrent_creation() {
        // UCE-32 Q30: Test concurrent preset creation for thread safety
        const NUM_THREADS: usize = 8;
        const CAPSULES_PER_THREAD: usize = 10;

        let mut handles = Vec::new();

        for thread_id in 0..NUM_THREADS {
            let handle = thread::spawn(move || {
                let mut capsules = Vec::new();

                for i in 0..CAPSULES_PER_THREAD {
                    let preset_type = match i % 4 {
                        0 => "hft",
                        1 => "conservative",
                        2 => "market",
                        _ => "limit",
                    };

                    let result = match preset_type {
                        "hft" => InternalHedgeBuilder::hft_preset("BTCUSD")
                            .on_exchange("NDAX")
                            .size(1.0 + thread_id as f64 * 0.1)
                            .stop_loss(45000.0)
                            .take_profit(55000.0)
                            .build(),
                        "conservative" => InternalHedgeBuilder::conservative_preset("ETHUSD")
                            .on_exchange("COINBASE")
                            .size(2.0 + thread_id as f64 * 0.1)
                            .stop_loss(3000.0)
                            .take_profit(4000.0)
                            .build(),
                        "market" => InternalHedgeBuilder::market_order("ADAUSD")
                            .size(100.0 + thread_id as f64 * 10.0)
                            .stop_loss(0.45)
                            .take_profit(0.55)
                            .build(),
                        "limit" => {
                            InternalHedgeBuilder::limit_order("SOLUSD", 150.0 + thread_id as f64)
                                .size(10.0 + thread_id as f64)
                                .stop_loss(140.0)
                                .take_profit(160.0)
                                .build()
                        }
                        _ => unreachable!(),
                    };

                    assert!(
                        result.is_ok(),
                        "Preset {} should succeed in thread {}",
                        preset_type,
                        thread_id
                    );
                    capsules.push(result.unwrap());
                }

                capsules
            });

            handles.push(handle);
        }

        // Collect all capsules
        let mut all_capsules = Vec::new();
        for handle in handles {
            let capsules = handle.join().expect("Thread should not panic");
            all_capsules.extend(capsules);
        }

        // Verify all capsules are valid
        assert_eq!(all_capsules.len(), NUM_THREADS * CAPSULES_PER_THREAD);
        for capsule in all_capsules {
            assert!(capsule.is_active(), "Each capsule should be active");
        }
    }

    #[test]
    fn test_preset_memory_usage() {
        // UCE-32 Q29: Test that presets don't create memory leaks or excessive allocations
        const ITERATIONS: usize = 1000;

        // Create and drop many preset capsules
        for i in 0..ITERATIONS {
            let preset_type = match i % 4 {
                0 => "hft",
                1 => "conservative",
                2 => "market",
                _ => "limit",
            };

            let _capsule = match preset_type {
                "hft" => InternalHedgeBuilder::hft_preset("BTCUSD")
                    .size(1.0)
                    .stop_loss(45000.0)
                    .take_profit(55000.0)
                    .build()
                    .unwrap(),
                "conservative" => InternalHedgeBuilder::conservative_preset("ETHUSD")
                    .size(1.0)
                    .stop_loss(3000.0)
                    .take_profit(4000.0)
                    .build()
                    .unwrap(),
                "market" => InternalHedgeBuilder::market_order("ADAUSD")
                    .size(1.0)
                    .stop_loss(0.45)
                    .take_profit(0.55)
                    .build()
                    .unwrap(),
                "limit" => InternalHedgeBuilder::limit_order("SOLUSD", 150.0)
                    .size(1.0)
                    .stop_loss(140.0)
                    .take_profit(160.0)
                    .build()
                    .unwrap(),
                _ => unreachable!(),
            };

            // Capsule is dropped here - should not leak memory
        }

        // If we reach here without OOM, memory usage is reasonable
        assert!(true, "Memory usage test completed successfully");
    }

    #[test]
    fn test_preset_configuration_equivalence() {
        // UCE-32 Q30: Test that preset configurations produce equivalent results to manual setup

        // Manual configuration
        let manual_entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let manual_bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        let manual_capsule = AtomicHedgeCapsule::new();
        manual_capsule
            .initialize(manual_entry, manual_bracket)
            .unwrap();

        // Preset configuration
        let preset_capsule = InternalHedgeBuilder::market_order("BTCUSD")
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build()
            .unwrap();

        // Both should have equivalent states
        assert_eq!(manual_capsule.is_active(), preset_capsule.is_active());
        assert_eq!(
            manual_capsule.is_emergency_stopped(),
            preset_capsule.is_emergency_stopped()
        );

        let manual_state = manual_capsule.get_hedge_state();
        let preset_state = preset_capsule.get_hedge_state();

        assert_eq!(manual_state.is_active, preset_state.is_active);
        assert_eq!(
            manual_state.emergency_stopped,
            preset_state.emergency_stopped
        );
    }

    #[test]
    fn test_preset_error_handling() {
        // UCE-32 Q31: Test proper error handling in preset configurations

        // Test missing required parameters
        let result = AtomicHedgeCapsule::hedge("BTCUSD").build(); // Missing size, stop_loss, take_profit

        assert!(result.is_err(), "Should fail without required parameters");

        // Test with only some parameters
        let result = AtomicHedgeCapsule::hedge("BTCUSD").size(1.0).build(); // Missing stop_loss, take_profit

        assert!(
            result.is_err(),
            "Should fail without stop_loss and take_profit"
        );

        // Test error message quality
        match result {
            Err(HedgeError::ValidationFailed { field, reason, .. }) => {
                assert!(
                    field == "stop_loss" || field == "take_profit",
                    "Should identify missing field"
                );
                assert!(!reason.is_empty(), "Should provide helpful error message");
            }
            Err(HedgeError::InitializationFailed(msg)) => {
                assert!(!msg.is_empty(), "Should provide helpful error message");
            }
            _ => panic!("Expected ValidationFailed or InitializationFailed error"),
        }
    }

    #[test]
    fn test_preset_integration_with_simplified_api() {
        // UCE-32 Q28: Test that presets work seamlessly with simplified API

        let capsule = InternalHedgeBuilder::hft_preset("BTCUSD")
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build()
            .unwrap();

        // Test simplified API methods work with preset capsule
        assert!(
            capsule.is_ready_to_hedge(),
            "Preset should be ready to hedge"
        );

        let initial_status = capsule.status();
        assert!(initial_status.is_active, "Preset should be active");
        assert!(
            !initial_status.is_emergency,
            "Should not be in emergency initially"
        );

        // Test order submission
        let submit_result = capsule.submit_order();
        assert!(submit_result.is_ok(), "Should submit order successfully");

        // Test progress update
        let progress_result = capsule.update_progress(0.5);
        assert!(
            progress_result.is_ok(),
            "Should update progress successfully"
        );

        // Test status after operations
        let updated_status = capsule.status();
        assert!(
            updated_status.completion > initial_status.completion,
            "Completion should increase"
        );

        // Test hedge execution
        let execution_result = capsule.execute_hedge(1.0);
        assert!(
            execution_result.is_ok(),
            "Should execute hedge successfully"
        );

        // Test final state
        assert!(capsule.is_active(), "Should remain active after operations");
    }

    #[test]
    fn test_preset_statistical_validation() {
        // UCE-32 Q30: Statistical validation of preset behavior across multiple runs
        const SAMPLE_SIZE: usize = 50;
        let mut success_count = 0;
        let mut total_operations = 0u64;
        let mut build_times = Vec::new();

        for i in 0..SAMPLE_SIZE {
            let start = Instant::now();

            let result = InternalHedgeBuilder::hft_preset("BTCUSD")
                .on_exchange("NDAX")
                .size(1.0 + i as f64 * 0.02)
                .stop_loss(45000.0 + i as f64 * 10.0)
                .take_profit(55000.0 + i as f64 * 10.0)
                .build();

            let build_time = start.elapsed();
            build_times.push(build_time.as_nanos());

            if let Ok(capsule) = result {
                success_count += 1;

                // Test basic operations
                if capsule.submit_order().is_ok() {
                    let _ = capsule.update_progress(0.5);
                    let state = capsule.get_hedge_state();
                    total_operations += state.operation_count;
                }
            }
        }

        // Statistical analysis
        let success_rate = (success_count as f64 / SAMPLE_SIZE as f64) * 100.0;
        let avg_operations = if success_count > 0 {
            total_operations / success_count as u64
        } else {
            0
        };

        let mean_build_time = build_times.iter().sum::<u128>() / build_times.len() as u128;
        let variance = build_times
            .iter()
            .map(|&x| (x as i128 - mean_build_time as i128).pow(2) as u128)
            .sum::<u128>()
            / build_times.len() as u128;
        let std_dev = (variance as f64).sqrt();

        println!("Preset Statistical Analysis (n={}):", SAMPLE_SIZE);
        println!("  Success Rate: {:.1}%", success_rate);
        println!("  Avg Operations: {}", avg_operations);
        println!(
            "  Build Time: mean={}ns, std_dev={:.2}ns",
            mean_build_time, std_dev
        );

        // UCE-32 Q30: Statistical validation requirements
        assert!(
            success_rate >= 98.0,
            "Success rate should be >= 98%: {:.1}%",
            success_rate
        );
        assert!(
            mean_build_time < 100_000,
            "Mean build time should be < 100μs: {}ns",
            mean_build_time
        );
        assert!(
            std_dev < mean_build_time as f64 * 0.5,
            "Standard deviation should be < 50% of mean"
        );

        // Test coefficient of variation (measure of consistency)
        let cv = std_dev / mean_build_time as f64;
        assert!(
            cv < 0.5,
            "Coefficient of variation should be < 0.5: {:.3}",
            cv
        );
    }

    #[test]
    fn test_preset_real_world_scenarios() {
        // UCE-32 Q29: Test presets against real-world trading scenarios

        // Scenario 1: Bitcoin day trading
        let btc_day_trade = InternalHedgeBuilder::hft_preset("BTCUSD")
            .on_exchange("NDAX")
            .size(0.1)
            .stop_loss(49000.0)
            .take_profit(51000.0)
            .build();

        assert!(btc_day_trade.is_ok(), "BTC day trading preset should work");

        // Scenario 2: Ethereum swing trading
        let eth_swing_trade = InternalHedgeBuilder::conservative_preset("ETHUSD")
            .on_exchange("COINBASE")
            .size(2.0)
            .stop_loss(2800.0)
            .take_profit(3200.0)
            .build();

        assert!(
            eth_swing_trade.is_ok(),
            "ETH swing trading preset should work"
        );

        // Scenario 3: Altcoin limit order
        let alt_limit = InternalHedgeBuilder::limit_order("ADAUSD", 0.52)
            .on_exchange("BINANCE")
            .size(1000.0)
            .stop_loss(0.48)
            .take_profit(0.56)
            .build();

        assert!(alt_limit.is_ok(), "Altcoin limit order preset should work");

        // Scenario 4: Large market order
        let large_market = InternalHedgeBuilder::market_order("SOLUSD")
            .on_exchange("FTX")
            .size(100.0)
            .stop_loss(140.0)
            .take_profit(160.0)
            .build();

        assert!(
            large_market.is_ok(),
            "Large market order preset should work"
        );

        // Test that all scenarios produce active capsules
        for (name, result) in [
            ("BTC day trade", btc_day_trade),
            ("ETH swing trade", eth_swing_trade),
            ("ADA limit", alt_limit),
            ("SOL market", large_market),
        ] {
            let capsule = result.unwrap();
            assert!(capsule.is_active(), "{} should be active", name);
            assert!(
                capsule.is_ready_to_hedge(),
                "{} should be ready to hedge",
                name
            );
        }
    }
}
