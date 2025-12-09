//! Logging Performance Benchmarks
//!
//! UCE32 Q30(Empirical Validation): Measure actual logging overhead to prove zero-cost abstractions
//! UCE32 Q31(Rust): Benchmark compile-time optimizations and conditional compilation

use atomic_hedge_capsule::logging::*;
use atomic_hedge_capsule::types::*;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::time::Duration;

fn bench_logging_disabled(c: &mut Criterion) {
    // UCE32 Q30: Measure overhead when logging is disabled
    set_logging_enabled(false);

    let mut group = c.benchmark_group("logging_disabled");
    group.throughput(Throughput::Elements(1));

    group.bench_function("simple_log", |b| {
        b.iter(|| {
            atomic_hedge_capsule::log_info!("Test message {}", 42);
        })
    });

    group.bench_function("structured_log", |b| {
        b.iter(|| {
            let mut fields = HashMap::new();
            fields.insert(
                "operation".to_string(),
                LogValue::String("test".to_string()),
            );
            fields.insert("latency".to_string(), LogValue::Integer(100));
            CapsuleLogger::log_with_fields(LogLevel::Info, "Test message", fields);
        })
    });

    group.bench_function("error_log", |b| {
        let error = HedgeError::timeout();
        b.iter(|| {
            CapsuleLogger::log_error(&error, "test_operation", Some("test context"));
        })
    });

    group.finish();
}

fn bench_logging_enabled(c: &mut Criterion) {
    // UCE32 Q30: Measure overhead when logging is enabled
    set_logging_enabled(true);
    set_log_level(LogLevel::Debug);

    let mut group = c.benchmark_group("logging_enabled");
    group.throughput(Throughput::Elements(1));

    group.bench_function("simple_log", |b| {
        b.iter(|| {
            atomic_hedge_capsule::log_info!("Test message {}", 42);
        })
    });

    group.bench_function("structured_log", |b| {
        b.iter(|| {
            let mut fields = HashMap::new();
            fields.insert(
                "operation".to_string(),
                LogValue::String("test".to_string()),
            );
            fields.insert("latency".to_string(), LogValue::Integer(100));
            CapsuleLogger::log_with_fields(LogLevel::Info, "Test message", fields);
        })
    });

    group.bench_function("state_transition_log", |b| {
        b.iter(|| {
            CapsuleLogger::log_state_transition(
                HedgeState::Idle,
                HedgeState::Active,
                Some(12345),
                Some(1000),
            );
        })
    });

    group.bench_function("error_log", |b| {
        let error = HedgeError::timeout();
        b.iter(|| {
            CapsuleLogger::log_error(&error, "test_operation", Some("test context"));
        })
    });

    group.bench_function("performance_log", |b| {
        let cache_metrics = CacheMetrics {
            hits: 100,
            misses: 10,
            hit_ratio: 0.91,
        };
        b.iter(|| {
            CapsuleLogger::log_performance("test_operation", 1000, 5, Some(cache_metrics.clone()));
        })
    });

    group.finish();
}

fn bench_log_levels(c: &mut Criterion) {
    // UCE32 Q31: Test zero-cost level filtering
    set_logging_enabled(true);

    let mut group = c.benchmark_group("log_levels");
    group.throughput(Throughput::Elements(1));

    // Test with different log levels to verify filtering overhead
    for level in [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ] {
        set_log_level(level);

        group.bench_with_input(
            BenchmarkId::new("filtered_trace", format!("{:?}", level)),
            &level,
            |b, _level| {
                b.iter(|| {
                    atomic_hedge_capsule::log_trace!("Trace message that should be filtered");
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("filtered_debug", format!("{:?}", level)),
            &level,
            |b, _level| {
                b.iter(|| {
                    atomic_hedge_capsule::log_debug!("Debug message that may be filtered");
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("filtered_info", format!("{:?}", level)),
            &level,
            |b, _level| {
                b.iter(|| {
                    atomic_hedge_capsule::log_info!("Info message");
                })
            },
        );
    }

    group.finish();
}

fn bench_formatting_performance(c: &mut Criterion) {
    // UCE32 Q32: Test SIMD formatting optimizations when available
    set_logging_enabled(true);
    set_log_level(LogLevel::Trace);

    let mut group = c.benchmark_group("formatting");
    group.throughput(Throughput::Elements(1));

    group.bench_function("format_timestamp", |b| {
        let timestamp_ns = 1640995200000000000u64; // 2022-01-01 00:00:00 UTC
        b.iter(|| {
            // Test timestamp formatting indirectly through log record creation
            let record = LogRecord {
                level: LogLevel::Info,
                message: "Timestamp test".to_string(),
                timestamp_ns,
                thread_id: 12345,
                operation_id: None,
                hedge_state: None,
                order_state: None,
                error_context: None,
                metrics: None,
                fields: HashMap::new(),
            };
            CapsuleLogger::format_record(&record)
        })
    });

    group.bench_function("format_simple_record", |b| {
        let record = LogRecord {
            level: LogLevel::Info,
            message: "Simple test message".to_string(),
            timestamp_ns: 1640995200000000000u64,
            thread_id: 12345,
            operation_id: Some(67890),
            hedge_state: None,
            order_state: None,
            error_context: None,
            metrics: None,
            fields: HashMap::new(),
        };

        b.iter(|| CapsuleLogger::format_record(&record))
    });

    group.bench_function("format_complex_record", |b| {
        let mut fields = HashMap::new();
        fields.insert(
            "operation".to_string(),
            LogValue::String("complex_test".to_string()),
        );
        fields.insert("latency_ns".to_string(), LogValue::Integer(1500));
        fields.insert("success".to_string(), LogValue::Boolean(true));
        fields.insert("price".to_string(), LogValue::Float(45123.67));

        let metrics = PerformanceMetrics {
            latency_ns: 1500,
            memory_ops: 8,
            cache_metrics: Some(CacheMetrics {
                hits: 150,
                misses: 12,
                hit_ratio: 0.926,
            }),
            contention_ns: Some(50),
        };

        let error_context = ErrorContext {
            category: ErrorCategory::Transient,
            recoverable: true,
            suggested_action: "Retry operation".to_string(),
            details: Some("Additional context information".to_string()),
        };

        let record = LogRecord {
            level: LogLevel::Debug,
            message: "Complex test message with lots of context".to_string(),
            timestamp_ns: 1640995200000000000u64,
            thread_id: 12345,
            operation_id: Some(67890),
            hedge_state: Some(HedgeState::Active),
            order_state: Some(OrderState::PartiallyFilled),
            error_context: Some(error_context),
            metrics: Some(metrics),
            fields,
        };

        b.iter(|| CapsuleLogger::format_record(&record))
    });

    group.finish();
}

fn bench_concurrent_logging(c: &mut Criterion) {
    // UCE32 Q31: Test lockfree performance under contention
    use std::sync::Arc;
    use std::thread;

    set_logging_enabled(true);
    set_log_level(LogLevel::Info);

    let mut group = c.benchmark_group("concurrent");
    group.throughput(Throughput::Elements(1));

    for thread_count in [1, 2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_simple", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|i| {
                            thread::spawn(move || {
                                atomic_hedge_capsule::log_info!(
                                    "Concurrent message from thread {}",
                                    i
                                );
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("concurrent_structured", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|i| {
                            thread::spawn(move || {
                                let mut fields = HashMap::new();
                                fields.insert("thread_id".to_string(), LogValue::Integer(i));
                                fields.insert(
                                    "operation".to_string(),
                                    LogValue::String("concurrent_test".to_string()),
                                );
                                CapsuleLogger::log_with_fields(
                                    LogLevel::Info,
                                    "Concurrent structured message",
                                    fields,
                                );
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

#[cfg(feature = "async")]
fn bench_async_logging(c: &mut Criterion) {
    use atomic_hedge_capsule::logging::async_logging::AsyncLogger;
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("async_logging");
    group.throughput(Throughput::Elements(1));

    group.bench_function("async_simple", |b| {
        let async_logger = AsyncLogger::new();

        b.to_async(&rt).iter(|| async {
            let record = LogRecord {
                level: LogLevel::Info,
                message: "Async test message".to_string(),
                timestamp_ns: 1640995200000000000u64,
                thread_id: 12345,
                operation_id: None,
                hedge_state: None,
                order_state: None,
                error_context: None,
                metrics: None,
                fields: HashMap::new(),
            };

            async_logger.log_async(record);
        })
    });

    group.finish();
}

// UCE32 Q30: Validate zero-cost abstractions empirically
fn bench_zero_cost_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_cost_validation");
    group.throughput(Throughput::Elements(1));

    // Baseline: no logging at all
    group.bench_function("baseline_no_logging", |b| {
        b.iter(|| {
            // Simulate work without any logging
            let _result = 42 * 2;
        })
    });

    // Logging disabled: should be identical to baseline
    group.bench_function("logging_disabled", |b| {
        set_logging_enabled(false);
        b.iter(|| {
            let _result = 42 * 2;
            atomic_hedge_capsule::log_info!("This should have zero cost");
        })
    });

    // Logging enabled but filtered out: should be minimal overhead
    group.bench_function("logging_filtered", |b| {
        set_logging_enabled(true);
        set_log_level(LogLevel::Error); // Filter out Info messages
        b.iter(|| {
            let _result = 42 * 2;
            atomic_hedge_capsule::log_info!("This should be filtered with minimal cost");
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_logging_disabled,
    bench_logging_enabled,
    bench_log_levels,
    bench_formatting_performance,
    bench_concurrent_logging,
    bench_zero_cost_validation,
    #[cfg(feature = "async")]
    bench_async_logging
);

criterion_main!(benches);
