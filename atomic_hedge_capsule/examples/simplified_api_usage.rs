//! Simplified AtomicHedgeCapsule API Usage Examples
//!
//! UCE-32 Q28 (Simplicity) - Demonstrating intuitive hedge operations
//!
//! These examples show how the simplified API makes hedge trading accessible
//! while maintaining all the sophisticated atomic coordination underneath.

use atomic_hedge_capsule::types::{ErrorCategory, HedgeError, HedgeResultExt};
use atomic_hedge_capsule::{AtomicHedgeCapsule, HedgeBuilder};

fn main() -> Result<(), HedgeError> {
    println!("AtomicHedgeCapsule Simplified API Examples");
    println!("==========================================\n");

    // Example 1: Quick hedge creation (simplest approach)
    println!("1. Quick Hedge Creation:");
    quick_hedge_example()?;

    // Example 2: Fluent builder pattern
    println!("\n2. Fluent Builder Pattern:");
    fluent_builder_example()?;

    // Example 3: Status monitoring
    println!("\n3. Status Monitoring:");
    status_monitoring_example()?;

    // Example 4: Error handling
    println!("\n4. Error Handling:");
    error_handling_example()?;

    // Example 5: Complete trading workflow
    println!("\n5. Complete Trading Workflow:");
    complete_workflow_example()?;

    println!("\n✅ All examples completed successfully!");
    Ok(())
}

/// Example 1: Quick hedge creation with minimal code
///
/// UCE-32 Q28: Single function call replaces complex initialization
fn quick_hedge_example() -> Result<(), HedgeError> {
    // Before: Complex EntryOrder + BracketOrder + initialize()
    // After: One simple function call
    let hedge = AtomicHedgeCapsule::create_hedge(
        "BTCUSD", // symbol
        "NDAX",   // exchange
        1.0,      // size
        45000.0,  // stop loss
        55000.0,  // take profit
    )?;

    println!(
        "   ✓ Created hedge for BTCUSD: stop={}, target={}",
        45000.0, 55000.0
    );
    println!("   ✓ Ready to trade: {}", hedge.is_ready_to_hedge());

    Ok(())
}

/// Example 2: Fluent builder for complex configurations
///
/// UCE-32 Q28: Readable, chainable configuration
fn fluent_builder_example() -> Result<(), HedgeError> {
    // Method 1: Step-by-step building
    let hedge1 = AtomicHedgeCapsule::hedge("ETHUSD")
        .on_exchange("Binance")
        .size(2.5)
        .stop_loss(3000.0)
        .take_profit(4000.0)
        .build()?;

    println!("   ✓ Built ETHUSD hedge on Binance");

    // Method 2: Using presets
    let hedge2 = HedgeBuilder::market_order("SOLUSD", 100.0)
        .size(10.0)
        .stop_loss(95.0)
        .take_profit(110.0)
        .build()?;

    println!("   ✓ Built SOLUSD market order hedge");

    // Method 3: One-line creation and submission
    let _hedge3 = AtomicHedgeCapsule::hedge("ADAUSD")
        .size(1000.0)
        .stop_loss(0.35)
        .take_profit(0.45)
        .build_and_submit()?;

    println!("   ✓ Created and submitted ADAUSD hedge in one step");

    Ok(())
}

/// Example 3: Simple status monitoring
///
/// UCE-32 Q28: Single status() call replaces complex state queries
fn status_monitoring_example() -> Result<(), HedgeError> {
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)?;

    // Submit and track progress
    hedge.submit_order()?;

    // Simple status checking
    let status = hedge.status();
    println!("   ✓ Hedge status: {}", status);
    println!("   ✓ Is safe: {}", status.is_safe());
    println!("   ✓ Needs attention: {}", status.needs_attention());

    // Simple boolean checks
    println!("   ✓ Is completed: {}", hedge.is_completed());
    println!("   ✓ Has errors: {}", hedge.has_errors());
    println!("   ✓ Is ready to hedge: {}", hedge.is_ready_to_hedge());

    // Progress simulation
    hedge.update_progress(0.5)?;
    println!("   ✓ Updated progress to 50%");

    let new_status = hedge.status();
    println!("   ✓ New completion: {:.1}%", new_status.completion * 100.0);

    Ok(())
}

/// Example 4: Simplified error handling
///
/// UCE-32 Q28: Clear error classification and guidance
fn error_handling_example() -> Result<(), HedgeError> {
    // Simulate various error scenarios
    let error_cases = vec![
        ("Timeout", HedgeError::timeout()),
        ("Emergency", HedgeError::emergency("Test emergency")),
        (
            "Invalid Value",
            HedgeError::invalid_value("size", "-1.0", "Size must be positive"),
        ),
        (
            "Out of Bounds",
            HedgeError::out_of_bounds(150.0, 0.0, 100.0),
        ),
    ];

    for (name, error) in error_cases {
        println!("   📋 Error Case: {}", name);
        println!("      • Recoverable: {}", error.is_recoverable());
        println!("      • Critical: {}", error.is_critical());
        println!("      • Category: {:?}", error.category());
        println!("      • Action: {}", error.suggested_action());

        // Using the result extension trait
        let result: Result<(), HedgeError> = Err(error);
        println!(
            "      • Should retry: {}",
            result.error_category().map_or(false, |c| c.should_retry())
        );
        println!();
    }

    Ok(())
}

/// Example 5: Complete trading workflow
///
/// UCE-32 Q28: Real-world usage with error handling
fn complete_workflow_example() -> Result<(), HedgeError> {
    println!("   🚀 Starting complete hedge workflow...");

    // Step 1: Create hedge with validation
    let hedge = match AtomicHedgeCapsule::hedge("BTCUSD")
        .on_exchange("NDAX")
        .size(0.1)
        .stop_loss(48000.0)
        .take_profit(52000.0)
        .build()
    {
        Ok(h) => {
            println!("   ✓ Hedge created successfully");
            h
        }
        Err(e) => {
            println!("   ❌ Failed to create hedge: {}", e.suggested_action());
            return Err(e);
        }
    };

    // Step 2: Submit order
    match hedge.submit_order() {
        Ok(_) => println!("   ✓ Order submitted"),
        Err(e) if e.is_recoverable() => {
            println!(
                "   ⚠️  Submission failed but recoverable: {}",
                e.suggested_action()
            );
            // In real code, could retry here
        }
        Err(e) => {
            println!("   ❌ Critical submission failure: {}", e);
            return Err(e);
        }
    }

    // Step 3: Monitor progress
    let mut fill_progress = 0.0;
    while fill_progress < 1.0 && !hedge.is_completed() {
        let status = hedge.status();

        if status.needs_attention() {
            println!("   ⚠️  Hedge needs attention: {}", status.description());

            if status.is_emergency {
                println!("   🛑 Emergency stop engaged - halting workflow");
                break;
            }
        }

        // Simulate progressive fill
        fill_progress += 0.25;
        if let Err(e) = hedge.update_progress(fill_progress) {
            if e.is_recoverable() {
                println!("   ⚠️  Progress update failed: {}", e.suggested_action());
                continue;
            } else {
                return Err(e);
            }
        }

        println!("   📈 Progress: {:.0}%", fill_progress * 100.0);
    }

    // Step 4: Final status
    let final_status = hedge.status();
    if final_status.completion >= 1.0 {
        println!("   ✅ Hedge completed successfully!");
        println!("      • Final size: {:.4}", final_status.filled_size);
        println!("      • Risk level: {:?}", final_status.risk_level);
    } else {
        println!("   ⏸️  Hedge workflow paused");
        println!(
            "      • Completion: {:.1}%",
            final_status.completion * 100.0
        );
    }

    // Step 5: Cleanup (if needed)
    if hedge.has_errors() {
        println!("   🔧 Resetting hedge due to errors");
        hedge.reset()?;
        println!("   ✓ Hedge reset to initial state");
    }

    Ok(())
}

/// Advanced usage examples
#[allow(dead_code)]
fn advanced_examples() -> Result<(), HedgeError> {
    // Batch operations
    let hedges: Result<Vec<_>, _> = ["BTCUSD", "ETHUSD", "SOLUSD"]
        .iter()
        .map(|symbol| {
            AtomicHedgeCapsule::hedge(symbol)
                .size(1.0)
                .stop_loss(1000.0)
                .take_profit(2000.0)
                .build()
        })
        .collect();

    let hedges = hedges?;
    println!("Created {} hedges", hedges.len());

    // Error recovery patterns
    for hedge in hedges {
        loop {
            match hedge.submit_order() {
                Ok(_) => break,
                Err(e) if e.is_recoverable() => {
                    println!("Retrying after recoverable error: {}", e.suggested_action());
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    Ok(())
}

// Performance comparison examples
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_simplified_vs_direct() {
        // Benchmark simplified API
        let start = Instant::now();
        for _ in 0..1000 {
            let _hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
                .expect("Hedge creation should succeed");
        }
        let simplified_duration = start.elapsed();

        // Benchmark direct API (for comparison)
        let start = Instant::now();
        for _ in 0..1000 {
            let capsule = AtomicHedgeCapsule::new();
            let entry = atomic_hedge_capsule::types::EntryOrder::new(
                "NDAX".to_string(),
                "BTCUSD".to_string(),
                "Buy".to_string(),
                1.0,
            );
            let bracket = atomic_hedge_capsule::types::BracketOrder::new(45000.0, 55000.0, 1.0);
            capsule
                .initialize(entry, bracket)
                .expect("Initialization should succeed");
        }
        let direct_duration = start.elapsed();

        println!("Simplified API: {:?}", simplified_duration);
        println!("Direct API: {:?}", direct_duration);

        // UCE-32 Q30: Empirical validation - simplified API should be comparable
        let overhead_ratio =
            simplified_duration.as_nanos() as f64 / direct_duration.as_nanos() as f64;
        assert!(
            overhead_ratio < 1.5,
            "Simplified API overhead should be < 50%: got {:.2}x",
            overhead_ratio
        );
    }
}
