//! Structured Logging Example for AtomicHedgeCapsule
//!
//! This example demonstrates comprehensive usage of the structured logging system.
//! UCE32 Q28(Simplicity): Simple, clear examples of logging in production scenarios.

use atomic_hedge_capsule::{
    // Import logging functionality
    init_logging,
    log_debug,
    log_error,
    log_info,
    log_trace,
    log_warn,
    set_log_level,
    AtomicHedgeCapsule,
    HedgeError,
    HedgeState,
    LogLevel,
};

use std::collections::HashMap;
use std::time::{Duration, Instant};

fn main() -> Result<(), HedgeError> {
    println!("=== AtomicHedgeCapsule Structured Logging Example ===\n");

    // Initialize logging with Info level
    init_logging(LogLevel::Info);

    log_info!("Starting AtomicHedgeCapsule logging demonstration");

    // Example 1: Basic logging levels
    demonstrate_log_levels()?;

    // Example 2: Structured logging with context
    demonstrate_structured_logging()?;

    // Example 3: Error logging with rich context
    demonstrate_error_logging()?;

    // Example 4: Performance logging
    demonstrate_performance_logging()?;

    // Example 5: State transition logging
    demonstrate_state_transitions()?;

    // Example 6: Production logging scenario
    demonstrate_production_scenario()?;

    log_info!("Logging demonstration completed successfully");
    Ok(())
}

fn demonstrate_log_levels() -> Result<(), HedgeError> {
    println!("\n--- Demonstrating Log Levels ---");

    log_error!("This is an ERROR message - critical failures");
    log_warn!("This is a WARN message - concerning conditions");
    log_info!("This is an INFO message - significant operations");

    // Set level to Debug to show debug/trace messages
    set_log_level(LogLevel::Debug);
    log_debug!("This is a DEBUG message - detailed operations");

    // Set level to Trace to show trace messages
    set_log_level(LogLevel::Trace);
    log_trace!("This is a TRACE message - every atomic operation");

    // Reset to Info level
    set_log_level(LogLevel::Info);

    Ok(())
}

fn demonstrate_structured_logging() -> Result<(), HedgeError> {
    use atomic_hedge_capsule::logging::{CapsuleLogger, LogValue};

    println!("\n--- Demonstrating Structured Logging ---");

    // Create structured fields
    let mut fields = HashMap::new();
    fields.insert("symbol".to_string(), LogValue::String("BTCUSD".to_string()));
    fields.insert("exchange".to_string(), LogValue::String("NDAX".to_string()));
    fields.insert("size".to_string(), LogValue::Float(1.5));
    fields.insert("stop_loss".to_string(), LogValue::Float(45000.0));
    fields.insert("take_profit".to_string(), LogValue::Float(55000.0));
    fields.insert("active".to_string(), LogValue::Boolean(true));

    CapsuleLogger::log_with_fields(LogLevel::Info, "Creating hedge with parameters", fields);

    // Performance metrics example
    let mut perf_fields = HashMap::new();
    perf_fields.insert(
        "operation".to_string(),
        LogValue::String("hedge_creation".to_string()),
    );
    perf_fields.insert("latency_ns".to_string(), LogValue::Integer(1250));
    perf_fields.insert("memory_ops".to_string(), LogValue::Integer(3));
    perf_fields.insert("cache_hit_ratio".to_string(), LogValue::Float(0.94));

    CapsuleLogger::log_with_fields(
        LogLevel::Debug,
        "Hedge creation performance metrics",
        perf_fields,
    );

    Ok(())
}

fn demonstrate_error_logging() -> Result<(), HedgeError> {
    use atomic_hedge_capsule::logging::CapsuleLogger;

    println!("\n--- Demonstrating Error Logging ---");

    // Simulate various error scenarios
    let timeout_error = HedgeError::timeout();
    CapsuleLogger::log_error(
        &timeout_error,
        "hedge_execution",
        Some("Network timeout during order submission"),
    );

    let validation_error = HedgeError::invalid_value("size", "0.0", "Size must be positive");
    CapsuleLogger::log_error(&validation_error, "parameter_validation", None);

    let emergency_error = HedgeError::emergency("Market volatility threshold exceeded");
    CapsuleLogger::log_error(
        &emergency_error,
        "risk_management",
        Some("Auto-stopping all active hedges"),
    );

    Ok(())
}

fn demonstrate_performance_logging() -> Result<(), HedgeError> {
    use atomic_hedge_capsule::logging::{CacheMetrics, CapsuleLogger};

    println!("\n--- Demonstrating Performance Logging ---");

    // Simulate operation timing
    let start = Instant::now();

    // Simulate some work
    std::thread::sleep(Duration::from_micros(100));

    let elapsed = start.elapsed();
    let latency_ns = elapsed.as_nanos() as u64;

    // Log performance without cache metrics
    CapsuleLogger::log_performance("order_submission", latency_ns, 5, None);

    // Log performance with cache metrics
    let cache_metrics = CacheMetrics {
        hits: 145,
        misses: 8,
        hit_ratio: 0.948,
    };

    CapsuleLogger::log_performance("state_update", latency_ns / 2, 3, Some(cache_metrics));

    Ok(())
}

fn demonstrate_state_transitions() -> Result<(), HedgeError> {
    use atomic_hedge_capsule::{logging::CapsuleLogger, HedgeState};

    println!("\n--- Demonstrating State Transition Logging ---");

    // Simulate hedge lifecycle
    let operation_id = 12345;

    CapsuleLogger::log_state_transition(
        HedgeState::Idle,
        HedgeState::Building,
        Some(operation_id),
        Some(850),
    );

    CapsuleLogger::log_state_transition(
        HedgeState::Building,
        HedgeState::Active,
        Some(operation_id),
        Some(1200),
    );

    CapsuleLogger::log_state_transition(
        HedgeState::Active,
        HedgeState::Unwinding,
        Some(operation_id),
        Some(950),
    );

    CapsuleLogger::log_state_transition(
        HedgeState::Unwinding,
        HedgeState::Idle,
        Some(operation_id),
        Some(750),
    );

    Ok(())
}

fn demonstrate_production_scenario() -> Result<(), HedgeError> {
    use atomic_hedge_capsule::logging::{CapsuleLogger, LogValue};

    println!("\n--- Demonstrating Production Scenario ---");

    log_info!("Starting production hedge scenario simulation");

    // Step 1: Initialize hedge
    log_info!("Initializing hedge configuration");
    let mut init_fields = HashMap::new();
    init_fields.insert(
        "session_id".to_string(),
        LogValue::String("prod_session_001".to_string()),
    );
    init_fields.insert("risk_limit".to_string(), LogValue::Float(10000.0));
    init_fields.insert("max_position".to_string(), LogValue::Float(5.0));

    CapsuleLogger::log_with_fields(
        LogLevel::Info,
        "Production session initialized",
        init_fields,
    );

    // Step 2: Create hedge capsule
    match AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0) {
        Ok(hedge) => {
            log_info!("Hedge capsule created successfully");

            // Step 3: Monitor execution
            let start = Instant::now();

            match hedge.submit_order() {
                Ok(_) => {
                    let submit_latency = start.elapsed().as_nanos() as u64;
                    CapsuleLogger::log_performance("order_submit", submit_latency, 2, None);

                    // Step 4: Execute hedge
                    match hedge.execute_hedge(1.0) {
                        Ok(result) => {
                            let mut result_fields = HashMap::new();
                            result_fields
                                .insert("success".to_string(), LogValue::Boolean(result.success));
                            result_fields.insert(
                                "entry_filled".to_string(),
                                LogValue::Float(result.entry_filled),
                            );
                            result_fields.insert(
                                "total_cost".to_string(),
                                LogValue::Float(result.total_cost),
                            );

                            CapsuleLogger::log_with_fields(
                                LogLevel::Info,
                                "Hedge execution completed",
                                result_fields,
                            );
                        }
                        Err(e) => {
                            CapsuleLogger::log_error(
                                &e,
                                "hedge_execution",
                                Some("Failed during execution phase"),
                            );
                        }
                    }
                }
                Err(e) => {
                    CapsuleLogger::log_error(
                        &e,
                        "order_submission",
                        Some("Failed to submit initial order"),
                    );
                }
            }
        }
        Err(e) => {
            CapsuleLogger::log_error(&e, "hedge_creation", Some("Failed to create hedge capsule"));
        }
    }

    log_info!("Production scenario simulation completed");
    Ok(())
}

#[cfg(feature = "async")]
async fn demonstrate_async_logging() -> Result<(), HedgeError> {
    use atomic_hedge_capsule::logging::async_logging::AsyncLogger;

    println!("\n--- Demonstrating Async Logging ---");

    let async_logger = AsyncLogger::new();

    // Create log records for async processing
    for i in 0..5 {
        let record = atomic_hedge_capsule::logging::LogRecord {
            level: LogLevel::Info,
            message: format!("Async log message {}", i),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            thread_id: 12345,
            operation_id: Some(i),
            hedge_state: None,
            order_state: None,
            error_context: None,
            metrics: None,
            fields: HashMap::new(),
        };

        async_logger.log_async(record);
    }

    // Give async logger time to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(())
}

#[cfg(feature = "async")]
#[tokio::main]
async fn async_main() -> Result<(), HedgeError> {
    main()?;
    demonstrate_async_logging().await?;
    Ok(())
}
