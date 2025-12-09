//! Backward Compatibility Tests for AtomicHedgeCapsule
//!
//! [TRADE SECRET] - Comprehensive validation of backward compatibility
//!
//! UCE-32 Q28(Simplicity): New simplified API must not break existing complex API usage
//! UCE-32 Q30(Validation): Statistical validation that all existing functionality works
//! UCE-32 Q31(Rust): Type-safe compatibility ensuring existing code compiles unchanged

use atomic_hedge_capsule::{
    AtomicHedgeCapsule, BracketOrder, EntryOrder, HedgeError, HedgeExecutionResult,
    HedgeStateSnapshot, HedgeStatus, OrderState,
};
use std::sync::Arc;
use std::thread;

/// Backward Compatibility Tests
///
/// These tests ensure that the new builder pattern and simplified API
/// do not break existing functionality or change existing behavior.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_original_constructor_still_works() {
        // UCE-32 Q28: Original AtomicHedgeCapsule::new() should still work
        let capsule = AtomicHedgeCapsule::new();
        assert!(!capsule.is_active(), "New capsule should be inactive");
        assert!(
            !capsule.is_emergency_stopped(),
            "New capsule should not be emergency stopped"
        );
    }

    #[test]
    fn test_original_initialization_workflow() {
        // UCE-32 Q30: Original initialization workflow should remain unchanged
        let capsule = AtomicHedgeCapsule::new();

        // Create entry and bracket orders manually (original way)
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );

        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);

        // Initialize manually (original way)
        let result = capsule.initialize(entry, bracket);
        assert!(result.is_ok(), "Original initialization should work");
        assert!(capsule.is_active(), "Should be active after initialization");
    }

    #[test]
    fn test_original_state_management() {
        // UCE-32 Q30: Original state management methods should work unchanged
        let capsule = AtomicHedgeCapsule::new();

        let entry = EntryOrder::new(
            "COINBASE".to_string(),
            "ETHUSD".to_string(),
            "Buy".to_string(),
            2.0,
        );

        let bracket = BracketOrder::new(3000.0, 4000.0, 2.0);
        capsule.initialize(entry, bracket).unwrap();

        // Test original state update methods
        let result = capsule.update_entry_state(OrderState::Validated, 0.0);
        assert!(result.is_ok(), "Original update_entry_state should work");

        let result = capsule.update_entry_state(OrderState::Submitted, 0.0);
        assert!(result.is_ok(), "Multiple state updates should work");

        let result = capsule.update_entry_state(OrderState::PartiallyFilled, 1.0);
        assert!(result.is_ok(), "Partial fill update should work");

        let result = capsule.update_entry_state(OrderState::Filled, 2.0);
        assert!(result.is_ok(), "Full fill update should work");
    }

    #[test]
    fn test_original_state_queries() {
        // UCE-32 Q30: Original state query methods should work unchanged
        let capsule = AtomicHedgeCapsule::new();

        let entry = EntryOrder::new(
            "BINANCE".to_string(),
            "ADAUSD".to_string(),
            "Buy".to_string(),
            1000.0,
        );

        let bracket = BracketOrder::new(0.45, 0.55, 1000.0);
        capsule.initialize(entry, bracket).unwrap();

        // Test all original query methods
        assert!(capsule.is_active(), "is_active should work");
        assert!(
            !capsule.is_emergency_stopped(),
            "is_emergency_stopped should work"
        );

        let state = capsule.get_hedge_state();
        assert!(state.is_active, "get_hedge_state should work");
        assert!(
            !state.emergency_stopped,
            "State should reflect non-emergency"
        );
        assert!(state.operation_count > 0, "Should track operations");

        // Test generation counter
        let gen1 = capsule.increment_generation();
        assert!(gen1.is_ok(), "increment_generation should work");

        let gen2 = capsule.increment_generation();
        assert!(gen2.is_ok(), "Multiple generation increments should work");
        assert!(gen2.unwrap() > gen1.unwrap(), "Generation should increase");
    }

    #[test]
    fn test_original_emergency_stop() {
        // UCE-32 Q30: Original emergency stop should work unchanged
        let capsule = AtomicHedgeCapsule::new();

        let entry = EntryOrder::new(
            "FTX".to_string(),
            "SOLUSD".to_string(),
            "Buy".to_string(),
            10.0,
        );

        let bracket = BracketOrder::new(140.0, 160.0, 10.0);
        capsule.initialize(entry, bracket).unwrap();

        // Test original emergency stop
        let result = capsule.emergency_stop("Test emergency stop");
        assert!(result.is_ok(), "Original emergency_stop should work");
        assert!(
            capsule.is_emergency_stopped(),
            "Should be emergency stopped"
        );

        // Operations should fail after emergency stop
        let update_result = capsule.update_entry_state(OrderState::Validated, 0.0);
        assert!(
            update_result.is_err(),
            "Updates should fail after emergency stop"
        );
    }

    #[test]
    fn test_original_two_phase_commit() {
        // UCE-32 Q30: Original two-phase commit should work unchanged
        let capsule = AtomicHedgeCapsule::new();

        let entry = EntryOrder::new(
            "KRAKEN".to_string(),
            "DOTUSD".to_string(),
            "Buy".to_string(),
            100.0,
        );

        let bracket = BracketOrder::new(20.0, 30.0, 100.0);
        capsule.initialize(entry, bracket).unwrap();

        // Test two-phase commit workflow
        let generation = capsule.prepare_update();
        assert!(generation.is_ok(), "prepare_update should work");

        let gen = generation.unwrap();
        let commit_result = capsule.commit_update(gen, OrderState::Validated, 50.0);
        assert!(commit_result.is_ok(), "commit_update should work");

        // Test rollback
        let gen2 = capsule.prepare_update().unwrap();
        let rollback_result = capsule.rollback_update(gen2);
        assert!(rollback_result.is_ok(), "rollback_update should work");
    }

    #[test]
    fn test_original_thread_safety() {
        // UCE-32 Q30: Original thread safety should be preserved
        let capsule = Arc::new(AtomicHedgeCapsule::new());

        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );

        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        capsule.initialize(entry, bracket).unwrap();

        const NUM_THREADS: usize = 4;
        const OPERATIONS_PER_THREAD: usize = 50;
        let mut handles = Vec::new();

        for thread_id in 0..NUM_THREADS {
            let capsule_clone = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                for i in 0..OPERATIONS_PER_THREAD {
                    match i % 4 {
                        0 => {
                            let _ = capsule_clone.update_entry_state(OrderState::Validated, 0.1);
                        }
                        1 => {
                            let _ = capsule_clone.get_hedge_state();
                        }
                        2 => {
                            let _ = capsule_clone.increment_generation();
                        }
                        3 => {
                            let _ = capsule_clone.is_active();
                        }
                        _ => unreachable!(),
                    }
                }
                thread_id
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread should not panic");
        }

        // Verify capsule remains in valid state
        assert!(
            capsule.is_active(),
            "Should remain active after concurrent operations"
        );
    }

    #[test]
    fn test_original_type_definitions() {
        // UCE-32 Q31: All original types should still be available and work

        // Test OrderState enum
        let order_state = OrderState::PendingValidation;
        assert_eq!(
            order_state as u32, 0,
            "OrderState values should be unchanged"
        );

        let validated = OrderState::Validated;
        assert_eq!(validated as u32, 1, "OrderState::Validated should be 1");

        // Test EntryOrder
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        assert_eq!(entry.size, 1.0, "EntryOrder fields should work");
        assert_eq!(entry.symbol, "BTCUSD", "EntryOrder symbol should work");
        assert!(entry.is_valid(), "EntryOrder validation should work");

        let entry_with_price = entry.with_price(50000.0);
        assert_eq!(
            entry_with_price.price,
            Some(50000.0),
            "with_price should work"
        );

        // Test BracketOrder
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        assert_eq!(
            bracket.stop_loss, 45000.0,
            "BracketOrder fields should work"
        );
        assert_eq!(
            bracket.take_profit, 55000.0,
            "BracketOrder take_profit should work"
        );
        assert!(
            bracket.is_valid().is_ok(),
            "BracketOrder validation should work"
        );

        let bracket_with_emergency = bracket.with_emergency_stop(40000.0);
        assert_eq!(
            bracket_with_emergency.emergency_stop,
            Some(40000.0),
            "with_emergency_stop should work"
        );

        // Test risk calculations
        let risk_reward = bracket.risk_reward_ratio(50000.0);
        assert!(risk_reward.is_some(), "Risk reward calculation should work");
    }

    #[test]
    fn test_original_error_handling() {
        // UCE-32 Q31: Original error types and handling should work unchanged
        let capsule = AtomicHedgeCapsule::new();

        // Test error when not initialized
        let update_result = capsule.update_entry_state(OrderState::Validated, 0.0);
        assert!(update_result.is_err(), "Should error when not initialized");

        match update_result.err().unwrap() {
            HedgeError::StateUpdateFailed(msg) => {
                assert!(!msg.is_empty(), "Should have error message");
            }
            _ => panic!("Expected StateUpdateFailed error"),
        }

        // Test invalid values
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        capsule.initialize(entry, bracket).unwrap();

        // Test invalid progress
        let invalid_progress = capsule.update_entry_state(OrderState::PartiallyFilled, -1.0);
        assert!(
            invalid_progress.is_err(),
            "Should reject negative filled amount"
        );
    }

    #[test]
    fn test_simplified_api_does_not_break_complex_api() {
        // UCE-32 Q28: Using simplified API should not interfere with complex API usage

        // Create using simplified API
        let simple_capsule =
            AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();

        // Should still be able to use complex API methods
        let state = simple_capsule.get_hedge_state();
        assert!(
            state.is_active,
            "Complex API should work on simplified capsule"
        );

        let gen = simple_capsule.increment_generation();
        assert!(gen.is_ok(), "Generation increment should work");

        let prepare_result = simple_capsule.prepare_update();
        assert!(prepare_result.is_ok(), "Two-phase commit should work");

        // Create using complex API
        let complex_capsule = AtomicHedgeCapsule::new();
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        complex_capsule.initialize(entry, bracket).unwrap();

        // Should be able to use simplified API methods
        let submit_result = complex_capsule.submit_order();
        assert!(
            submit_result.is_ok(),
            "Simplified API should work on complex capsule"
        );

        let status = complex_capsule.status();
        assert!(status.is_active, "Status method should work");

        let progress_result = complex_capsule.update_progress(0.5);
        assert!(progress_result.is_ok(), "Progress update should work");
    }

    #[test]
    fn test_performance_not_degraded() {
        // UCE-32 Q30: New APIs should not degrade performance of existing operations
        const ITERATIONS: usize = 1000;

        // Measure original API performance
        let start = std::time::Instant::now();
        for _i in 0..ITERATIONS {
            let capsule = AtomicHedgeCapsule::new();
            let entry = EntryOrder::new(
                "NDAX".to_string(),
                "BTCUSD".to_string(),
                "Buy".to_string(),
                1.0,
            );
            let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
            capsule.initialize(entry, bracket).unwrap();
            let _ = capsule.update_entry_state(OrderState::Validated, 0.0);
            let _ = capsule.get_hedge_state();
        }
        let original_duration = start.elapsed();

        // Measure simplified API performance
        let start = std::time::Instant::now();
        for _i in 0..ITERATIONS {
            let capsule =
                AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();
            let _ = capsule.submit_order();
            let _ = capsule.status();
        }
        let simplified_duration = start.elapsed();

        println!("Original API: {:?}", original_duration);
        println!("Simplified API: {:?}", simplified_duration);

        // Simplified API might be slightly faster due to optimizations,
        // but should not be significantly slower
        let ratio = simplified_duration.as_nanos() as f64 / original_duration.as_nanos() as f64;
        assert!(
            ratio < 2.0,
            "Simplified API should not be more than 2x slower: ratio={:.2}",
            ratio
        );
    }

    #[test]
    fn test_existing_test_patterns_still_work() {
        // UCE-32 Q30: Patterns from existing tests should continue to work

        // Pattern from existing concurrent tests
        let capsule = Arc::new(AtomicHedgeCapsule::new());
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        capsule.initialize(entry, bracket).unwrap();

        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for _i in 0..10 {
                let _ = capsule_clone.update_entry_state(OrderState::Validated, 0.1);
                let _ = capsule_clone.get_hedge_state();
            }
        });

        handle.join().unwrap();
        assert!(capsule.is_active(), "Concurrent pattern should still work");

        // Pattern from existing validation tests
        #[cfg(debug_assertions)]
        {
            assert!(
                capsule.validate_thread_safety(),
                "Thread safety validation should work"
            );
        }

        // Pattern from existing cache tests
        let cache_info = capsule.cache_info();
        assert_eq!(cache_info.alignment, 64, "Cache info should still work");

        let validation = cache_info.validate_cache_optimization();
        assert!(
            validation.is_cache_aligned,
            "Cache validation should still work"
        );
    }

    #[test]
    fn test_api_surface_compatibility() {
        // UCE-32 Q31: All public APIs should remain available with same signatures

        let capsule = AtomicHedgeCapsule::new();

        // Test that all original methods are still callable with same signatures
        let _active = capsule.is_active();
        let _emergency = capsule.is_emergency_stopped();
        let _state = capsule.get_hedge_state();

        // Test builder methods work alongside original
        let _builder_capsule = AtomicHedgeCapsule::hedge("BTCUSD")
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build()
            .unwrap();

        // Test that types are still available
        let _order_state: OrderState = OrderState::PendingValidation;
        let _hedge_error: HedgeError = HedgeError::Timeout;

        // Test that re-exports work
        use atomic_hedge_capsule::{AtomicHedgeCapsule as AHC, HedgeExecutionResult, HedgeStatus};
        let _capsule: AHC = AtomicHedgeCapsule::new();
        let _status: HedgeStatus = capsule.status();
    }

    #[test]
    fn test_documentation_examples_still_compile() {
        // UCE-32 Q28: Documentation examples should continue to work

        // Example from lib.rs documentation
        let hedge =
            AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();

        hedge.submit_order().unwrap();
        let result = hedge.execute_hedge(1.0).unwrap();
        assert!(result.success);

        // Fluent builder example
        let hedge2 = AtomicHedgeCapsule::hedge("ETHUSD")
            .on_exchange("NDAX")
            .size(2.5)
            .stop_loss(3000.0)
            .take_profit(4000.0)
            .build()
            .unwrap();

        let status = hedge2.status();
        assert!(!status.description().is_empty());

        // Error handling example
        match hedge.execute_hedge(1.0) {
            Ok(result) => assert!(result.success),
            Err(e) if e.is_recoverable() => {
                assert!(!e.suggested_action().is_empty());
            }
            Err(_e) => {
                // Critical error handling
            }
        }
    }
}
