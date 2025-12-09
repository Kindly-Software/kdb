//! Comprehensive Test Coverage for Structured Logging
//!
//! UCE32 Q30(Empirical Validation): Complete test coverage proving logging system correctness
//! UCE32 Q28(Simplicity): Clear, focused tests that validate all functionality

#[cfg(feature = "logging")]
mod logging_tests {
    use atomic_hedge_capsule::logging::*;
    use atomic_hedge_capsule::types::*;
    use atomic_hedge_capsule::{HedgeError, HedgeState};
    use std::collections::HashMap;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_log_levels() {
        // Test log level ordering
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Trace);

        // Test should_log logic
        assert!(LogLevel::Error.should_log(LogLevel::Error));
        assert!(LogLevel::Error.should_log(LogLevel::Warn));
        assert!(LogLevel::Error.should_log(LogLevel::Info));
        assert!(LogLevel::Error.should_log(LogLevel::Debug));
        assert!(LogLevel::Error.should_log(LogLevel::Trace));

        assert!(!LogLevel::Warn.should_log(LogLevel::Error));
        assert!(LogLevel::Warn.should_log(LogLevel::Warn));

        assert!(!LogLevel::Debug.should_log(LogLevel::Info));
        assert!(LogLevel::Debug.should_log(LogLevel::Debug));
    }

    #[test]
    fn test_log_level_conversions() {
        assert_eq!(LogLevel::from(0), LogLevel::Error);
        assert_eq!(LogLevel::from(1), LogLevel::Warn);
        assert_eq!(LogLevel::from(2), LogLevel::Info);
        assert_eq!(LogLevel::from(3), LogLevel::Debug);
        assert_eq!(LogLevel::from(99), LogLevel::Trace); // Default for invalid values

        assert_eq!(LogLevel::Error.as_str(), "ERROR");
        assert_eq!(LogLevel::Warn.as_str(), "WARN ");
        assert_eq!(LogLevel::Info.as_str(), "INFO ");
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
        assert_eq!(LogLevel::Trace.as_str(), "TRACE");
    }

    #[test]
    fn test_log_config() {
        let config = LogConfig::new();

        // Test default values
        assert_eq!(config.level(), LogLevel::Info);
        assert!(config.is_enabled());
        assert!(config.colors_enabled());

        // Test level setting
        config.set_level(LogLevel::Debug);
        assert_eq!(config.level(), LogLevel::Debug);

        config.set_level(LogLevel::Error);
        assert_eq!(config.level(), LogLevel::Error);

        // Test enable/disable
        config.set_enabled(false);
        assert!(!config.is_enabled());

        config.set_enabled(true);
        assert!(config.is_enabled());

        // Test should_log
        config.set_level(LogLevel::Info);
        config.set_enabled(true);
        assert!(config.should_log(LogLevel::Error));
        assert!(config.should_log(LogLevel::Warn));
        assert!(config.should_log(LogLevel::Info));
        assert!(!config.should_log(LogLevel::Debug));
        assert!(!config.should_log(LogLevel::Trace));

        // Test disabled state
        config.set_enabled(false);
        assert!(!config.should_log(LogLevel::Error));
        assert!(!config.should_log(LogLevel::Info));
    }

    #[test]
    fn test_log_values() {
        // Test string values
        let str_val = LogValue::String("test".to_string());
        assert_eq!(str_val.format(), "test");

        // Test integer values
        let int_val = LogValue::Integer(42);
        assert_eq!(int_val.format(), "42");

        let neg_int_val = LogValue::Integer(-123);
        assert_eq!(neg_int_val.format(), "-123");

        // Test float values
        let float_val = LogValue::Float(3.14159);
        assert!(float_val.format().starts_with("3.14159"));

        // Test boolean values
        let bool_true = LogValue::Boolean(true);
        assert_eq!(bool_true.format(), "true");

        let bool_false = LogValue::Boolean(false);
        assert_eq!(bool_false.format(), "false");

        // Test duration values
        let duration_val = LogValue::Duration(Duration::from_nanos(1500));
        assert_eq!(duration_val.format(), "1500ns");

        // Test state values
        let state_val = LogValue::State("Active".to_string());
        assert_eq!(state_val.format(), "Active");
    }

    #[test]
    fn test_log_value_conversions() {
        // Test From implementations
        let str_val: LogValue = "test string".into();
        assert_eq!(str_val.format(), "test string");

        let string_val: LogValue = String::from("owned string").into();
        assert_eq!(string_val.format(), "owned string");

        let int_val: LogValue = 42i64.into();
        assert_eq!(int_val.format(), "42");

        let uint_val: LogValue = 123u64.into();
        assert_eq!(uint_val.format(), "123");

        let float_val: LogValue = 3.14f64.into();
        assert!(float_val.format().starts_with("3.14"));

        let bool_val: LogValue = true.into();
        assert_eq!(bool_val.format(), "true");

        let duration_val: LogValue = Duration::from_millis(100).into();
        assert_eq!(duration_val.format(), "100000000ns");

        let hedge_state_val: LogValue = HedgeState::Active.into();
        assert_eq!(hedge_state_val.format(), "Active");

        let order_state_val: LogValue = OrderState::Filled.into();
        assert_eq!(order_state_val.format(), "Filled");
    }

    #[test]
    fn test_error_context() {
        let error = HedgeError::timeout();

        let context = ErrorContext {
            category: error.category(),
            recoverable: error.is_recoverable(),
            suggested_action: error.suggested_action().to_string(),
            details: Some("Network timeout during order submission".to_string()),
        };

        assert_eq!(context.category, ErrorCategory::Transient);
        assert!(context.recoverable);
        assert_eq!(
            context.suggested_action,
            "Retry operation with longer timeout"
        );
        assert!(context.details.is_some());
    }

    #[test]
    fn test_performance_metrics() {
        let cache_metrics = CacheMetrics {
            hits: 100,
            misses: 10,
            hit_ratio: 0.91,
        };

        let perf_metrics = PerformanceMetrics {
            latency_ns: 1500,
            memory_ops: 5,
            cache_metrics: Some(cache_metrics),
            contention_ns: Some(250),
        };

        assert_eq!(perf_metrics.latency_ns, 1500);
        assert_eq!(perf_metrics.memory_ops, 5);
        assert!(perf_metrics.cache_metrics.is_some());
        assert_eq!(perf_metrics.cache_metrics.as_ref().unwrap().hit_ratio, 0.91);
        assert_eq!(perf_metrics.contention_ns, Some(250));
    }

    #[test]
    fn test_log_record_creation() {
        let mut fields = HashMap::new();
        fields.insert(
            "operation".to_string(),
            LogValue::String("test".to_string()),
        );
        fields.insert("latency".to_string(), LogValue::Integer(1000));

        let record = LogRecord {
            level: LogLevel::Info,
            message: "Test message".to_string(),
            timestamp_ns: 1640995200000000000,
            thread_id: 12345,
            operation_id: Some(67890),
            hedge_state: Some(HedgeState::Active),
            order_state: Some(OrderState::Filled),
            error_context: None,
            metrics: None,
            fields,
        };

        assert_eq!(record.level, LogLevel::Info);
        assert_eq!(record.message, "Test message");
        assert_eq!(record.thread_id, 12345);
        assert_eq!(record.operation_id, Some(67890));
        assert_eq!(record.hedge_state, Some(HedgeState::Active));
        assert_eq!(record.order_state, Some(OrderState::Filled));
        assert_eq!(record.fields.len(), 2);
    }

    #[test]
    fn test_simple_logging_macros() {
        // These tests verify macros compile correctly
        // The actual output would be tested through integration tests

        // Enable logging for testing
        set_logging_enabled(true);
        set_log_level(LogLevel::Trace);

        atomic_hedge_capsule::log_error!("Error message: {}", "test error");
        atomic_hedge_capsule::log_warn!("Warning message: {}", 42);
        atomic_hedge_capsule::log_info!("Info message");
        atomic_hedge_capsule::log_debug!("Debug message with value: {}", 3.14);
        atomic_hedge_capsule::log_trace!("Trace message");

        // Test with no arguments
        atomic_hedge_capsule::log_info!("Simple message without formatting");

        // Reset logging state
        set_log_level(LogLevel::Info);
    }

    #[test]
    fn test_structured_logging() {
        set_logging_enabled(true);
        set_log_level(LogLevel::Debug);

        let mut fields = HashMap::new();
        fields.insert("symbol".to_string(), LogValue::String("BTCUSD".to_string()));
        fields.insert("size".to_string(), LogValue::Float(1.5));
        fields.insert("success".to_string(), LogValue::Boolean(true));

        // This should not panic
        CapsuleLogger::log_with_fields(LogLevel::Info, "Structured test message", fields);

        // Test empty fields
        let empty_fields = HashMap::new();
        CapsuleLogger::log_with_fields(LogLevel::Debug, "Empty fields test", empty_fields);
    }

    #[test]
    fn test_error_logging() {
        set_logging_enabled(true);
        set_log_level(LogLevel::Error);

        let timeout_error = HedgeError::timeout();
        CapsuleLogger::log_error(&timeout_error, "test_operation", Some("test context"));

        let validation_error = HedgeError::invalid_value("size", "0.0", "Must be positive");
        CapsuleLogger::log_error(&validation_error, "validation", None);

        let emergency_error = HedgeError::emergency("Market volatility");
        CapsuleLogger::log_error(
            &emergency_error,
            "risk_management",
            Some("Auto-stop triggered"),
        );
    }

    #[test]
    fn test_state_transition_logging() {
        set_logging_enabled(true);
        set_log_level(LogLevel::Info);

        CapsuleLogger::log_state_transition(
            HedgeState::Idle,
            HedgeState::Active,
            Some(12345),
            Some(1500),
        );

        CapsuleLogger::log_state_transition(HedgeState::Active, HedgeState::Emergency, None, None);
    }

    #[test]
    fn test_performance_logging() {
        set_logging_enabled(true);
        set_log_level(LogLevel::Debug);

        // Without cache metrics
        CapsuleLogger::log_performance("test_operation", 1000, 3, None);

        // With cache metrics
        let cache_metrics = CacheMetrics {
            hits: 95,
            misses: 5,
            hit_ratio: 0.95,
        };
        CapsuleLogger::log_performance("cached_operation", 500, 2, Some(cache_metrics));
    }

    #[test]
    fn test_format_record() {
        let mut fields = HashMap::new();
        fields.insert(
            "test_field".to_string(),
            LogValue::String("test_value".to_string()),
        );

        let record = LogRecord {
            level: LogLevel::Info,
            message: "Test formatting".to_string(),
            timestamp_ns: 1640995200000000000,
            thread_id: 12345,
            operation_id: Some(67890),
            hedge_state: None,
            order_state: None,
            error_context: None,
            metrics: None,
            fields,
        };

        let formatted = CapsuleLogger::format_record(&record);

        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("Test formatting"));
        assert!(formatted.contains("T:12345"));
        assert!(formatted.contains("OP:67890"));
        assert!(formatted.contains("test_field=test_value"));
    }

    #[test]
    fn test_format_record_with_metrics() {
        let cache_metrics = CacheMetrics {
            hits: 100,
            misses: 5,
            hit_ratio: 0.95,
        };

        let metrics = PerformanceMetrics {
            latency_ns: 1500,
            memory_ops: 3,
            cache_metrics: Some(cache_metrics),
            contention_ns: None,
        };

        let record = LogRecord {
            level: LogLevel::Debug,
            message: "Performance test".to_string(),
            timestamp_ns: 1640995200000000000,
            thread_id: 54321,
            operation_id: None,
            hedge_state: None,
            order_state: None,
            error_context: None,
            metrics: Some(metrics),
            fields: HashMap::new(),
        };

        let formatted = CapsuleLogger::format_record(&record);

        assert!(formatted.contains("DEBUG"));
        assert!(formatted.contains("Performance test"));
        assert!(formatted.contains("PERF: 1500ns"));
        assert!(formatted.contains("3mem_ops"));
        assert!(formatted.contains("cache_hit_ratio=0.95"));
    }

    #[test]
    fn test_format_record_with_error_context() {
        let error_context = ErrorContext {
            category: ErrorCategory::Transient,
            recoverable: true,
            suggested_action: "Retry operation".to_string(),
            details: Some("Network issue".to_string()),
        };

        let record = LogRecord {
            level: LogLevel::Error,
            message: "Error occurred".to_string(),
            timestamp_ns: 1640995200000000000,
            thread_id: 98765,
            operation_id: None,
            hedge_state: None,
            order_state: None,
            error_context: Some(error_context),
            metrics: None,
            fields: HashMap::new(),
        };

        let formatted = CapsuleLogger::format_record(&record);

        assert!(formatted.contains("ERROR"));
        assert!(formatted.contains("Error occurred"));
        assert!(formatted.contains("Transient"));
        assert!(formatted.contains("recoverable=true"));
        assert!(formatted.contains("Retry operation"));
    }

    #[test]
    fn test_concurrent_logging() {
        set_logging_enabled(true);
        set_log_level(LogLevel::Info);

        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    for j in 0..10 {
                        atomic_hedge_capsule::log_info!("Thread {} message {}", i, j);

                        let mut fields = HashMap::new();
                        fields.insert("thread".to_string(), LogValue::Integer(i));
                        fields.insert("iteration".to_string(), LogValue::Integer(j));

                        CapsuleLogger::log_with_fields(
                            LogLevel::Debug,
                            "Concurrent structured message",
                            fields,
                        );
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_logging_disabled_overhead() {
        // Test that disabled logging has minimal overhead
        set_logging_enabled(false);

        let start = std::time::Instant::now();

        for i in 0..1000 {
            atomic_hedge_capsule::log_info!("Disabled message {}", i);
        }

        let disabled_time = start.elapsed();

        // This should be very fast since logging is disabled
        assert!(
            disabled_time < Duration::from_millis(10),
            "Disabled logging took too long: {:?}",
            disabled_time
        );
    }

    #[test]
    fn test_level_filtering() {
        set_logging_enabled(true);

        // Set to Error level only
        set_log_level(LogLevel::Error);

        // These should not be logged (we can't easily test the output,
        // but we can ensure they don't panic)
        atomic_hedge_capsule::log_warn!("Should be filtered");
        atomic_hedge_capsule::log_info!("Should be filtered");
        atomic_hedge_capsule::log_debug!("Should be filtered");
        atomic_hedge_capsule::log_trace!("Should be filtered");

        // This should be logged
        atomic_hedge_capsule::log_error!("Should be logged");

        // Test with structured logging
        let mut fields = HashMap::new();
        fields.insert("filtered".to_string(), LogValue::Boolean(true));

        CapsuleLogger::log_with_fields(LogLevel::Debug, "Should be filtered", fields);
    }

    #[test]
    fn test_configuration_functions() {
        // Test init_logging
        init_logging(LogLevel::Warn);
        assert_eq!(current_log_level(), LogLevel::Warn);
        assert!(is_logging_enabled());

        // Test individual settings
        set_log_level(LogLevel::Debug);
        assert_eq!(current_log_level(), LogLevel::Debug);

        set_logging_enabled(false);
        assert!(!is_logging_enabled());

        set_logging_enabled(true);
        assert!(is_logging_enabled());

        set_colors_enabled(false);
        // Color setting doesn't have a getter, but function should not panic

        set_colors_enabled(true);
        // Reset to default state
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_async_logging() {
        use atomic_hedge_capsule::logging::async_logging::AsyncLogger;

        let async_logger = AsyncLogger::new();

        // Create test records
        for i in 0..5 {
            let record = LogRecord {
                level: LogLevel::Info,
                message: format!("Async test message {}", i),
                timestamp_ns: 1640995200000000000,
                thread_id: 12345,
                operation_id: Some(i as u64),
                hedge_state: None,
                order_state: None,
                error_context: None,
                metrics: None,
                fields: HashMap::new(),
            };

            async_logger.log_async(record);
        }

        // Give async logger time to process
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// Tests that should always run, regardless of features
#[test]
fn test_logging_feature_detection() {
    // This test verifies the feature detection works
    #[cfg(feature = "logging")]
    {
        assert!(atomic_hedge_capsule::features::has_logging());
    }

    #[cfg(not(feature = "logging"))]
    {
        assert!(!atomic_hedge_capsule::features::has_logging());
    }
}

#[test]
fn test_hedge_error_logging_helpers() {
    // Test error helper methods that are used in logging
    let timeout_error = HedgeError::timeout();
    assert!(timeout_error.is_recoverable());
    assert!(!timeout_error.is_critical());
    assert_eq!(timeout_error.category(), ErrorCategory::Transient);

    let emergency_error = HedgeError::emergency("Test emergency");
    assert!(!emergency_error.is_recoverable());
    assert!(emergency_error.is_critical());
    assert_eq!(emergency_error.category(), ErrorCategory::Operational);

    let validation_error = HedgeError::invalid_value("test", "invalid", "reason");
    assert!(!validation_error.is_recoverable());
    assert!(!validation_error.is_critical());
    assert_eq!(validation_error.category(), ErrorCategory::Configuration);
}

#[test]
fn test_error_category_helpers() {
    // Test ErrorCategory methods
    assert!(ErrorCategory::Transient.should_retry());
    assert!(!ErrorCategory::Configuration.should_retry());
    assert!(!ErrorCategory::Operational.should_retry());
    assert!(!ErrorCategory::System.should_retry());

    assert_eq!(
        ErrorCategory::Transient.description(),
        "Temporary issue - retry recommended"
    );
    assert_eq!(
        ErrorCategory::Configuration.description(),
        "Configuration error - fix inputs"
    );
    assert_eq!(
        ErrorCategory::Operational.description(),
        "Operational issue - check state"
    );
    assert_eq!(
        ErrorCategory::System.description(),
        "System error - contact support"
    );
}
