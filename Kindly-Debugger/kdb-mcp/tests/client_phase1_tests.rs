//! T28 Q1-Q7 Unit Tests for Phase 1 Client Modules
//!
//! Comprehensive test coverage for MCP client resilience capsules:
//! - McpMetricsCapsule (T1+T3): Atomic counters + Q16.16 fixed-point latency
//! - MutableRetryConfig (T1): Exponential backoff with atomic attempt tracking
//! - MutableCircuitBreaker (T1): State machine with lockfree transitions
//!
//! ## Test Organization (T28 Framework)
//!
//! - Q1: Capsule Layout Verification (size, alignment, cache line)
//! - Q2: Initialization & Defaults (new(), from_env(), Default trait)
//! - Q3: Core Operations (record, update, state transitions)
//! - Q4: Error Handling (exhaustion, rejection, boundary conditions)
//! - Q5: State Consistency (concurrent access, race conditions)
//! - Q6: Edge Cases (empty, zero, overflow, boundary values)
//! - Q7: Integration (full pipeline, cascading failures)
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_CACHE_ALIGNMENT`: 64B alignment prevents false sharing
//! - `#VERIFY_CACHE_ALIGNMENT`: static_assert in compile-time tests
//! - `#ASSUME_MONOTONIC_COUNTERS`: Counters only increment, Relaxed ordering safe
//! - `#VERIFY_MONOTONIC`: All counter updates use fetch_add
//! - `#ASSUME_LOCKFREE`: 100% lockfree, no mutex/RwLock
//! - `#VERIFY_LOCKFREE`: Only AtomicU8/AtomicU64 operations used

use std::sync::Arc;
use std::thread;

use kdb_mcp::client::McpMetricsCapsule;

#[cfg(feature = "client-retry")]
use kdb_mcp::client::{
    BackoffStrategy, MutableRetryConfig, is_retryable_error, retry_http_request,
};

#[cfg(feature = "client-circuit-breaker")]
use kdb_mcp::client::{
    CircuitBreakerError, CircuitBreakerState, MutableCircuitBreaker,
};

// =============================================================================
// Q1: Capsule Layout Verification
// =============================================================================

mod q1_layout_verification {
    use super::*;

    /// Q1.1: Verify McpMetricsCapsule is exactly 128 bytes (2 cache lines)
    #[test]
    fn q1_metrics_capsule_size() {
        assert_eq!(
            std::mem::size_of::<McpMetricsCapsule>(),
            128,
            "McpMetricsCapsule must be exactly 128 bytes (2 cache lines)"
        );
    }

    /// Q1.2: Verify McpMetricsCapsule has 64-byte alignment
    #[test]
    fn q1_metrics_capsule_alignment() {
        assert_eq!(
            std::mem::align_of::<McpMetricsCapsule>(),
            64,
            "McpMetricsCapsule must be 64-byte aligned (cache line)"
        );
    }

    /// Q1.3: Verify McpMetricsCapsule heap allocation alignment
    #[test]
    fn q1_metrics_heap_alignment() {
        let metrics = Box::new(McpMetricsCapsule::new());
        let ptr = &*metrics as *const McpMetricsCapsule as usize;
        assert_eq!(
            ptr % 64,
            0,
            "Heap-allocated McpMetricsCapsule must be 64-byte aligned"
        );
    }

    /// Q1.4: Verify MutableRetryConfig size and alignment
    ///
    /// Note: Documentation says 64 bytes but actual size is 128 bytes due to
    /// RetryPolicy alignment requirements. The important property is 64-byte alignment.
    #[test]
    #[cfg(feature = "client-retry")]
    fn q1_retry_config_size() {
        let size = std::mem::size_of::<MutableRetryConfig>();
        // Actual size is 128 bytes (2 cache lines)
        // Important: must be a multiple of 64 for cache alignment
        assert!(
            size % 64 == 0,
            "MutableRetryConfig size ({}) must be a multiple of 64 for cache alignment",
            size
        );
        assert_eq!(
            size, 128,
            "MutableRetryConfig is 128 bytes (documentation says 64, but RetryPolicy requires more)"
        );
    }

    /// Q1.5: Verify MutableRetryConfig has 64-byte alignment
    #[test]
    #[cfg(feature = "client-retry")]
    fn q1_retry_config_alignment() {
        assert_eq!(
            std::mem::align_of::<MutableRetryConfig>(),
            64,
            "MutableRetryConfig must be 64-byte aligned (cache line)"
        );
    }

    /// Q1.6: Verify MutableCircuitBreaker size and alignment
    ///
    /// Note: Documentation says 64 bytes but actual size is 128 bytes due to
    /// inner CircuitBreaker alignment requirements. The important property is 64-byte alignment.
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q1_circuit_breaker_size() {
        let size = std::mem::size_of::<MutableCircuitBreaker>();
        // Actual size is 128 bytes (2 cache lines)
        // Important: must be a multiple of 64 for cache alignment
        assert!(
            size % 64 == 0,
            "MutableCircuitBreaker size ({}) must be a multiple of 64 for cache alignment",
            size
        );
        assert_eq!(
            size, 128,
            "MutableCircuitBreaker is 128 bytes (documentation says 64, but inner CircuitBreaker requires more)"
        );
    }

    /// Q1.7: Verify MutableCircuitBreaker has 64-byte alignment
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q1_circuit_breaker_alignment() {
        assert_eq!(
            std::mem::align_of::<MutableCircuitBreaker>(),
            64,
            "MutableCircuitBreaker must be 64-byte aligned (cache line)"
        );
    }

    /// Q1.8: Verify MutableCircuitBreaker heap allocation alignment
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q1_circuit_breaker_heap_alignment() {
        let cb = Box::new(MutableCircuitBreaker::default());
        let ptr = &*cb as *const MutableCircuitBreaker as usize;
        assert_eq!(
            ptr % 64,
            0,
            "Heap-allocated MutableCircuitBreaker must be 64-byte aligned"
        );
    }
}

// =============================================================================
// Q2: Initialization & Defaults
// =============================================================================

mod q2_initialization {
    use super::*;

    /// Q2.1: Verify McpMetricsCapsule initializes all counters to zero
    #[test]
    fn q2_metrics_initialization() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        let stats = metrics.get_stats();

        assert_eq!(stats.total_requests, 0, "total_requests should be 0");
        assert_eq!(stats.successful_requests, 0, "successful_requests should be 0");
        assert_eq!(stats.failed_requests, 0, "failed_requests should be 0");
        assert_eq!(stats.cached_hits, 0, "cached_hits should be 0");
        assert_eq!(stats.retried_requests, 0, "retried_requests should be 0");
        assert_eq!(stats.circuit_breaker_rejects, 0, "circuit_breaker_rejects should be 0");
        assert_eq!(stats.average_latency_us, 0.0, "average_latency_us should be 0.0");
        assert_eq!(stats.max_latency_us, 0.0, "max_latency_us should be 0.0");
        assert_eq!(stats.p99_latency_us, 0.0, "p99_latency_us should be 0.0");
        assert_eq!(stats.success_rate, 1.0, "success_rate should be 1.0 (no requests)");
        assert_eq!(stats.started_at_unix, 1000, "started_at_unix should match");
    }

    /// Q2.2: Verify McpMetricsCapsule Default trait
    #[test]
    fn q2_metrics_default_impl() {
        let metrics = McpMetricsCapsule::default();
        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 0);
        // started_at_unix should be current timestamp (non-zero in std mode)
    }

    /// Q2.3: Verify MutableRetryConfig default values
    #[test]
    #[cfg(feature = "client-retry")]
    fn q2_retry_config_defaults() {
        let config = MutableRetryConfig::default();
        assert_eq!(config.max_retries(), 5, "Default max_retries should be 5");
        assert_eq!(config.current_attempt(), 0, "Initial current_attempt should be 0");
        assert!(!config.is_exhausted(), "Should not be exhausted initially");
    }

    /// Q2.4: Verify MutableRetryConfig from environment variables
    #[test]
    #[cfg(feature = "client-retry")]
    fn q2_retry_config_from_env() {
        // Set environment variables
        std::env::set_var("KDB_RETRY_MAX", "10");
        std::env::set_var("KDB_RETRY_BACKOFF", "persistent");

        let config = MutableRetryConfig::from_env();
        assert_eq!(config.max_retries(), 10, "Should read KDB_RETRY_MAX from env");

        // Clean up
        std::env::remove_var("KDB_RETRY_MAX");
        std::env::remove_var("KDB_RETRY_BACKOFF");
    }

    /// Q2.5: Verify MutableRetryConfig from_env with defaults when vars not set
    #[test]
    #[cfg(feature = "client-retry")]
    fn q2_retry_config_from_env_defaults() {
        // Clear environment variables
        std::env::remove_var("KDB_RETRY_MAX");
        std::env::remove_var("KDB_RETRY_BACKOFF");

        let config = MutableRetryConfig::from_env();
        assert_eq!(config.max_retries(), 5, "Should default to 5 retries");
    }

    /// Q2.6: Verify MutableCircuitBreaker default values
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q2_circuit_breaker_defaults() {
        let cb = MutableCircuitBreaker::default();
        assert_eq!(cb.failure_threshold(), 5, "Default failure_threshold should be 5");
        assert_eq!(cb.recovery_timeout_secs(), 60, "Default recovery_timeout should be 60s");
        assert_eq!(cb.half_open_success_threshold(), 3, "Default half_open_success_threshold should be 3");
        assert!(cb.is_closed(), "Should be closed initially");
        assert_eq!(cb.failure_count(), 0, "Initial failure_count should be 0");
    }

    /// Q2.7: Verify MutableCircuitBreaker from environment variables
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q2_circuit_breaker_from_env() {
        std::env::set_var("KDB_CB_FAILURE_THRESHOLD", "10");
        std::env::set_var("KDB_CB_RECOVERY_TIMEOUT", "120");
        std::env::set_var("KDB_CB_HALF_OPEN_SUCCESS", "5");

        let cb = MutableCircuitBreaker::from_env();
        assert_eq!(cb.failure_threshold(), 10);
        assert_eq!(cb.recovery_timeout_secs(), 120);
        assert_eq!(cb.half_open_success_threshold(), 5);

        // Clean up
        std::env::remove_var("KDB_CB_FAILURE_THRESHOLD");
        std::env::remove_var("KDB_CB_RECOVERY_TIMEOUT");
        std::env::remove_var("KDB_CB_HALF_OPEN_SUCCESS");
    }

    /// Q2.8: Verify MutableCircuitBreaker from_env with defaults
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q2_circuit_breaker_from_env_defaults() {
        std::env::remove_var("KDB_CB_FAILURE_THRESHOLD");
        std::env::remove_var("KDB_CB_RECOVERY_TIMEOUT");
        std::env::remove_var("KDB_CB_HALF_OPEN_SUCCESS");

        let cb = MutableCircuitBreaker::from_env();
        assert_eq!(cb.failure_threshold(), 5);
        assert_eq!(cb.recovery_timeout_secs(), 60);
        assert_eq!(cb.half_open_success_threshold(), 3);
    }
}

// =============================================================================
// Q3: Core Operations
// =============================================================================

mod q3_core_operations {
    use super::*;

    /// Q3.1: Verify metrics record_request for successful request
    #[test]
    fn q3_metrics_record_successful_request() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1500, false, 0);

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.cached_hits, 0);
        assert_eq!(stats.retried_requests, 0);
        assert!((stats.average_latency_us - 1500.0).abs() < 0.1);
        assert_eq!(stats.success_rate, 1.0);
    }

    /// Q3.2: Verify metrics record_request for failed request
    #[test]
    fn q3_metrics_record_failed_request() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(false, 5000, false, 0);

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.success_rate, 0.0);
    }

    /// Q3.3: Verify metrics record_request tracks cache hits
    #[test]
    fn q3_metrics_cache_hit_tracking() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 100, true, 0);
        metrics.record_request(true, 1000, false, 0);
        metrics.record_request(true, 50, true, 0);

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.cached_hits, 2);
    }

    /// Q3.4: Verify metrics record_request tracks retries
    #[test]
    fn q3_metrics_retry_tracking() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1000, false, 0);  // No retries
        metrics.record_request(true, 2000, false, 2);  // 2 retries
        metrics.record_request(false, 3000, false, 5); // 5 retries

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.retried_requests, 7); // 0 + 2 + 5
    }

    /// Q3.5: Verify metrics record_circuit_breaker_reject
    #[test]
    fn q3_metrics_circuit_breaker_reject() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1000, false, 0);
        metrics.record_circuit_breaker_reject();
        metrics.record_circuit_breaker_reject();

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 2);
        assert_eq!(stats.circuit_breaker_rejects, 2);
    }

    /// Q3.6: Verify Q16.16 fixed-point latency conversion accuracy
    #[test]
    fn q3_metrics_q16_16_conversion_accuracy() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1000, false, 0); // 1ms
        metrics.record_request(true, 2000, false, 0); // 2ms
        metrics.record_request(true, 3000, false, 0); // 3ms

        let stats = metrics.get_stats();

        // Average should be 2000us
        assert!(
            (stats.average_latency_us - 2000.0).abs() < 1.0,
            "Average latency: {} (expected ~2000)",
            stats.average_latency_us
        );

        // Max should be 3000us
        assert!(
            (stats.max_latency_us - 3000.0).abs() < 1.0,
            "Max latency: {} (expected ~3000)",
            stats.max_latency_us
        );
    }

    /// Q3.7: Verify retry backoff calculation
    #[test]
    #[cfg(feature = "client-retry")]
    fn q3_retry_backoff_calculation() {
        // With IMMEDIATE strategy, delay should be 0
        let config_immediate = MutableRetryConfig::new(BackoffStrategy::IMMEDIATE, 5);
        assert_eq!(config_immediate.next_delay_ms(), 0);
    }

    /// Q3.8: Verify retry attempt increment
    #[test]
    #[cfg(feature = "client-retry")]
    fn q3_retry_attempt_increment() {
        let config = MutableRetryConfig::new(BackoffStrategy::STANDARD, 5);

        assert_eq!(config.current_attempt(), 0);
        assert_eq!(config.increment_attempt(), 0); // Returns previous value
        assert_eq!(config.current_attempt(), 1);
        assert_eq!(config.increment_attempt(), 1);
        assert_eq!(config.current_attempt(), 2);
    }

    /// Q3.9: Verify circuit breaker Closed -> Open transition
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q3_circuit_breaker_closed_to_open() {
        let cb = MutableCircuitBreaker::new(3, 60);

        assert!(cb.is_closed());

        // Record failures up to threshold
        cb.record_failure_silent();
        assert!(cb.is_closed());
        assert_eq!(cb.failure_count(), 1);

        cb.record_failure_silent();
        assert!(cb.is_closed());
        assert_eq!(cb.failure_count(), 2);

        cb.record_failure_silent();
        assert!(cb.is_open());
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    /// Q3.10: Verify circuit breaker Open -> HalfOpen transition
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q3_circuit_breaker_open_to_half_open() {
        let cb = MutableCircuitBreaker::new(1, 1);

        // Open the circuit
        cb.record_failure_silent();
        assert!(cb.is_open());

        // Manually trigger recovery
        cb.try_recovery();

        assert!(cb.is_half_open());
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
    }

    /// Q3.11: Verify circuit breaker HalfOpen -> Closed success transition
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q3_circuit_breaker_half_open_to_closed() {
        let cb = MutableCircuitBreaker::with_half_open_threshold(1, 1, 2);

        // Open and transition to half-open
        cb.record_failure_silent();
        cb.try_recovery();
        assert!(cb.is_half_open());

        // Record successes to meet threshold
        cb.record_success();
        assert!(cb.is_half_open()); // Not yet at threshold
        assert_eq!(cb.half_open_success_count(), 1);

        cb.record_success();
        assert!(cb.is_closed()); // Threshold met
        assert_eq!(cb.failure_count(), 0);
    }

    /// Q3.12: Verify circuit breaker HalfOpen -> Open failure transition
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q3_circuit_breaker_half_open_to_open_failure() {
        let cb = MutableCircuitBreaker::new(1, 60);

        // Open and transition to half-open
        cb.record_failure_silent();
        cb.try_recovery();
        assert!(cb.is_half_open());

        // Failure during half-open re-opens
        cb.record_failure_silent();
        assert!(cb.is_open());
    }
}

// =============================================================================
// Q4: Error Handling
// =============================================================================

mod q4_error_handling {
    use super::*;

    /// Q4.1: Verify retry exhaustion
    #[test]
    #[cfg(feature = "client-retry")]
    fn q4_retry_exhaustion() {
        let config = MutableRetryConfig::new(BackoffStrategy::IMMEDIATE, 3);

        assert!(!config.is_exhausted());
        config.increment_attempt(); // 0 -> 1
        assert!(!config.is_exhausted());
        config.increment_attempt(); // 1 -> 2
        assert!(!config.is_exhausted());
        config.increment_attempt(); // 2 -> 3
        assert!(config.is_exhausted()); // 3 >= 3
    }

    /// Q4.2: Verify retry_http_request exhaustion behavior
    #[test]
    #[cfg(feature = "client-retry")]
    fn q4_retry_http_request_exhaustion() {
        let mut config = MutableRetryConfig::new(BackoffStrategy::IMMEDIATE, 2);

        let result: Result<&str, std::io::Error> = retry_http_request(&mut config, || {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "always fails"))
        });

        assert!(result.is_err());
    }

    /// Q4.3: Verify circuit breaker rejection when open
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q4_circuit_breaker_rejection() {
        let cb = MutableCircuitBreaker::new(1, 3600); // Long recovery timeout

        // Open the circuit
        cb.record_failure_silent();
        assert!(cb.is_open());

        // Check should fail
        assert!(matches!(
            cb.check_no_time(),
            Err(CircuitBreakerError::Open)
        ));
    }

    /// Q4.4: Verify circuit breaker ForcedOpen rejection
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q4_circuit_breaker_forced_open_rejection() {
        let cb = MutableCircuitBreaker::new(5, 60);

        // Initially closed
        assert!(cb.check_no_time().is_ok());

        // Force open
        cb.force_open();

        assert_eq!(cb.state(), CircuitBreakerState::ForcedOpen);
        assert!(matches!(
            cb.check_no_time(),
            Err(CircuitBreakerError::ForcedOpen)
        ));
    }

    /// Q4.5: Verify is_retryable_error for 5xx errors
    #[test]
    #[cfg(feature = "client-retry")]
    fn q4_retryable_5xx_errors() {
        assert!(is_retryable_error(500)); // Internal Server Error
        assert!(is_retryable_error(501)); // Not Implemented
        assert!(is_retryable_error(502)); // Bad Gateway
        assert!(is_retryable_error(503)); // Service Unavailable
        assert!(is_retryable_error(504)); // Gateway Timeout
        assert!(is_retryable_error(599)); // Custom 5xx
    }

    /// Q4.6: Verify is_retryable_error for special cases
    #[test]
    #[cfg(feature = "client-retry")]
    fn q4_retryable_special_cases() {
        assert!(is_retryable_error(429)); // Rate Limited
        assert!(is_retryable_error(408)); // Request Timeout
    }

    /// Q4.7: Verify non-retryable 4xx errors
    #[test]
    #[cfg(feature = "client-retry")]
    fn q4_non_retryable_4xx_errors() {
        assert!(!is_retryable_error(400)); // Bad Request
        assert!(!is_retryable_error(401)); // Unauthorized
        assert!(!is_retryable_error(403)); // Forbidden
        assert!(!is_retryable_error(404)); // Not Found
        assert!(!is_retryable_error(422)); // Unprocessable Entity
    }

    /// Q4.8: Verify non-retryable success codes
    #[test]
    #[cfg(feature = "client-retry")]
    fn q4_non_retryable_success_codes() {
        assert!(!is_retryable_error(200)); // OK
        assert!(!is_retryable_error(201)); // Created
        assert!(!is_retryable_error(204)); // No Content
    }
}

// =============================================================================
// Q5: State Consistency (Concurrent Access)
// =============================================================================

mod q5_state_consistency {
    use super::*;

    /// Q5.1: Verify metrics concurrent updates safety
    #[test]
    fn q5_metrics_concurrent_updates() {
        let metrics = Arc::new(McpMetricsCapsule::with_timestamp(1000));
        let mut handles = vec![];

        // Spawn 10 threads, each recording 100 requests
        for _ in 0..10 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    m.record_request(
                        i % 2 == 0,           // 50% success
                        (i * 10) as u32,      // Varying latency
                        i % 3 == 0,           // ~33% cache hits
                        (i % 4) as u8,        // Varying retries
                    );
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 1000);
        assert_eq!(stats.successful_requests, 500); // 50% of 1000
        assert_eq!(stats.failed_requests, 500);
    }

    /// Q5.2: Verify metrics circuit breaker reject concurrent access
    #[test]
    fn q5_metrics_concurrent_circuit_breaker_rejects() {
        let metrics = Arc::new(McpMetricsCapsule::with_timestamp(1000));
        let mut handles = vec![];

        // Spawn threads recording circuit breaker rejects
        for _ in 0..10 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    m.record_circuit_breaker_reject();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let stats = metrics.get_stats();
        assert_eq!(stats.circuit_breaker_rejects, 500);
        assert_eq!(stats.total_requests, 500);
        assert_eq!(stats.failed_requests, 500);
    }

    /// Q5.3: Verify circuit breaker concurrent failure recording
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q5_circuit_breaker_concurrent_failures() {
        let cb = Arc::new(MutableCircuitBreaker::new(100, 60));
        let mut handles = vec![];

        // Spawn 10 threads, each recording 10 failures
        for _ in 0..10 {
            let b = Arc::clone(&cb);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    b.record_failure_silent();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // All 100 failures recorded, circuit should be open
        assert!(cb.is_open());
    }

    /// Q5.4: Verify retry config atomic attempt increment
    #[test]
    #[cfg(feature = "client-retry")]
    fn q5_retry_concurrent_attempt_increment() {
        let config = Arc::new(MutableRetryConfig::new(BackoffStrategy::IMMEDIATE, 255));
        let mut handles = vec![];

        // Spawn 10 threads, each incrementing 10 times
        for _ in 0..10 {
            let c = Arc::clone(&config);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    c.increment_attempt();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(config.current_attempt(), 100);
    }

    /// Q5.5: Verify max latency tracking under concurrent access
    #[test]
    fn q5_metrics_concurrent_max_latency() {
        let metrics = Arc::new(McpMetricsCapsule::with_timestamp(1000));
        let mut handles = vec![];

        // Spawn threads with varying latencies
        for thread_id in 0..10 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    // Each thread uses a different latency range
                    let latency = ((thread_id * 1000) + i * 10) as u32;
                    m.record_request(true, latency, false, 0);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let stats = metrics.get_stats();
        // Max should be from thread 9: 9000 + 990 = 9990
        assert!(
            stats.max_latency_us >= 9000.0,
            "Max latency should be >= 9000, got {}",
            stats.max_latency_us
        );
    }
}

// =============================================================================
// Q6: Edge Cases
// =============================================================================

mod q6_edge_cases {
    use super::*;

    /// Q6.1: Verify metrics empty stats success rate
    #[test]
    fn q6_metrics_empty_stats_success_rate() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        // Empty metrics should return 1.0 (100%) success rate
        assert_eq!(metrics.success_rate(), 1.0);
    }

    /// Q6.2: Verify metrics empty stats average latency
    #[test]
    fn q6_metrics_empty_stats_average_latency() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        // Empty metrics should return 0.0 average latency (no division by zero)
        assert_eq!(metrics.average_latency_us(), 0.0);
    }

    /// Q6.3: Verify metrics with zero latency
    #[test]
    fn q6_metrics_zero_latency() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        metrics.record_request(true, 0, false, 0);

        let stats = metrics.get_stats();
        assert_eq!(stats.average_latency_us, 0.0);
        assert_eq!(stats.max_latency_us, 0.0);
    }

    /// Q6.4: Verify metrics with max u32 latency
    #[test]
    fn q6_metrics_max_latency() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        metrics.record_request(true, u32::MAX, false, 0);

        let stats = metrics.get_stats();
        // Should not overflow (u64 * 65536 fits in u64 for u32::MAX)
        assert!(stats.max_latency_us > 0.0);
        assert!(stats.average_latency_us > 0.0);
    }

    /// Q6.5: Verify retry with zero retries
    #[test]
    #[cfg(feature = "client-retry")]
    fn q6_retry_zero_retries() {
        let config = MutableRetryConfig::new(BackoffStrategy::STANDARD, 0);
        assert!(config.is_exhausted()); // 0 >= 0
    }

    /// Q6.6: Verify retry with max u8 retries
    #[test]
    #[cfg(feature = "client-retry")]
    fn q6_retry_max_retries() {
        let config = MutableRetryConfig::new(BackoffStrategy::STANDARD, 255);
        assert_eq!(config.max_retries(), 255);
        assert!(!config.is_exhausted());
    }

    /// Q6.7: Verify circuit breaker with threshold 1
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q6_circuit_breaker_threshold_one() {
        let cb = MutableCircuitBreaker::new(1, 60);
        assert!(cb.is_closed());

        cb.record_failure_silent();
        assert!(cb.is_open()); // Opens immediately on first failure
    }

    /// Q6.8: Verify circuit breaker with threshold 255
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q6_circuit_breaker_max_threshold() {
        let cb = MutableCircuitBreaker::new(255, 60);
        assert!(cb.is_closed());

        // Record 254 failures - should still be closed
        for _ in 0..254 {
            cb.record_failure_silent();
        }
        assert!(cb.is_closed());
        assert_eq!(cb.failure_count(), 254);

        // 255th failure opens
        cb.record_failure_silent();
        assert!(cb.is_open());
    }

    /// Q6.9: Verify circuit breaker success resets failures in closed state
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q6_circuit_breaker_success_resets_failures() {
        let cb = MutableCircuitBreaker::new(5, 60);

        // Accumulate some failures
        cb.record_failure_silent();
        cb.record_failure_silent();
        assert_eq!(cb.failure_count(), 2);

        // Success should reset
        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert!(cb.is_closed());
    }

    /// Q6.10: Verify circuit breaker reset clears all state
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q6_circuit_breaker_reset() {
        let cb = MutableCircuitBreaker::new(1, 60);

        // Open the circuit
        cb.record_failure_silent();
        assert!(cb.is_open());

        // Reset
        cb.reset();

        assert!(cb.is_closed());
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.half_open_success_count(), 0);
        assert!(cb.check_no_time().is_ok());
    }

    /// Q6.11: Verify retry config clone preserves state
    #[test]
    #[cfg(feature = "client-retry")]
    fn q6_retry_config_clone() {
        let config = MutableRetryConfig::new(BackoffStrategy::LIGHT, 7);
        config.increment_attempt();
        config.increment_attempt();

        let cloned = config.clone();
        assert_eq!(cloned.max_retries(), 7);
        assert_eq!(cloned.current_attempt(), 2);
    }

    /// Q6.12: Verify retry config reset
    #[test]
    #[cfg(feature = "client-retry")]
    fn q6_retry_config_reset() {
        let mut config = MutableRetryConfig::new(BackoffStrategy::STANDARD, 5);
        config.increment_attempt();
        config.increment_attempt();
        assert_eq!(config.current_attempt(), 2);

        config.reset();
        assert_eq!(config.current_attempt(), 0);
    }
}

// =============================================================================
// Q7: Integration (Full Pipeline)
// =============================================================================

mod q7_integration {
    use super::*;

    /// Q7.1: Full pipeline success - metrics + retry working together
    #[test]
    #[cfg(all(feature = "client-retry", feature = "client-circuit-breaker"))]
    fn q7_full_pipeline_success() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        let cb = MutableCircuitBreaker::new(5, 60);
        let mut config = MutableRetryConfig::new(BackoffStrategy::IMMEDIATE, 3);
        let mut attempt_count = 0;

        // Simulate retry behavior
        let result = retry_http_request(&mut config, || {
            attempt_count += 1;
            if attempt_count < 2 {
                // Record failure and check circuit breaker
                cb.record_failure_silent();
                metrics.record_request(false, 1000, false, 1);
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"))
            } else {
                // Success
                cb.record_success();
                metrics.record_request(true, 500, false, 0);
                Ok("success")
            }
        });

        assert!(result.is_ok());
        assert!(cb.is_closed());

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 1);
    }

    /// Q7.2: Full pipeline failure - circuit breaker opens after retries
    #[test]
    #[cfg(all(feature = "client-retry", feature = "client-circuit-breaker"))]
    fn q7_full_pipeline_circuit_breaker_opens() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        let cb = MutableCircuitBreaker::new(3, 60);
        let mut config = MutableRetryConfig::new(BackoffStrategy::IMMEDIATE, 5);

        // Retry will fail and open circuit breaker
        let _result: Result<&str, std::io::Error> = retry_http_request(&mut config, || {
            cb.record_failure_silent();
            metrics.record_request(false, 1000, false, 0);
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "always fails"))
        });

        // Circuit breaker should be open after 3 failures
        assert!(cb.is_open());

        // Further requests should be rejected
        assert!(matches!(cb.check_no_time(), Err(CircuitBreakerError::Open)));
    }

    /// Q7.3: Cascading failure handling
    #[test]
    #[cfg(all(feature = "client-retry", feature = "client-circuit-breaker"))]
    fn q7_cascading_failure_handling() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        let cb = MutableCircuitBreaker::new(2, 60);

        // Simulate cascading failures
        for _ in 0..5 {
            if cb.check_no_time().is_ok() {
                cb.record_failure_silent();
                metrics.record_request(false, 1000, false, 0);
            } else {
                metrics.record_circuit_breaker_reject();
            }
        }

        let stats = metrics.get_stats();
        // 2 failures before open, then 3 rejections
        assert_eq!(stats.failed_requests, 5);
        assert_eq!(stats.circuit_breaker_rejects, 3);
    }

    /// Q7.4: Recovery after circuit breaker trip
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn q7_recovery_after_trip() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        let cb = MutableCircuitBreaker::with_half_open_threshold(2, 1, 1);

        // Trip the circuit
        cb.record_failure_silent();
        cb.record_failure_silent();
        assert!(cb.is_open());
        metrics.record_request(false, 1000, false, 0);

        // Manual recovery transition
        cb.try_recovery();
        assert!(cb.is_half_open());

        // Success in half-open closes the circuit
        cb.record_success();
        assert!(cb.is_closed());
        metrics.record_request(true, 500, false, 0);

        let stats = metrics.get_stats();
        assert_eq!(stats.successful_requests, 1);
    }

    /// Q7.5: Metrics accumulation across multiple scenarios
    #[test]
    fn q7_metrics_comprehensive_accumulation() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        // Simulate various request scenarios
        // Successful requests with varying latencies
        for i in 0..10 {
            metrics.record_request(true, (100 + i * 50) as u32, false, 0);
        }

        // Failed requests
        for _ in 0..5 {
            metrics.record_request(false, 2000, false, 0);
        }

        // Cache hits
        for _ in 0..8 {
            metrics.record_request(true, 10, true, 0);
        }

        // Retried requests
        for retry_count in 1..4 {
            metrics.record_request(true, 1000, false, retry_count);
        }

        // Circuit breaker rejections
        metrics.record_circuit_breaker_reject();
        metrics.record_circuit_breaker_reject();

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 28); // 10 + 5 + 8 + 3 + 2
        assert_eq!(stats.successful_requests, 21); // 10 + 8 + 3
        assert_eq!(stats.failed_requests, 7); // 5 + 2
        assert_eq!(stats.cached_hits, 8);
        assert_eq!(stats.retried_requests, 6); // 1 + 2 + 3
        assert_eq!(stats.circuit_breaker_rejects, 2);
        assert!(stats.success_rate > 0.7 && stats.success_rate < 0.8);
    }

    /// Q7.6: Success rate calculation accuracy
    #[test]
    fn q7_success_rate_accuracy() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        // 7 successful, 3 failed = 70% success rate
        for _ in 0..7 {
            metrics.record_request(true, 100, false, 0);
        }
        for _ in 0..3 {
            metrics.record_request(false, 100, false, 0);
        }

        let rate = metrics.success_rate();
        assert!(
            (rate - 0.7).abs() < 0.001,
            "Success rate: {} (expected 0.7)",
            rate
        );
    }

    /// Q7.7: P99 latency estimation under mixed load
    #[test]
    fn q7_p99_latency_estimation() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        // Record mostly low latencies
        for _ in 0..99 {
            metrics.record_request(true, 100, false, 0);
        }
        // Record one high latency (outlier)
        metrics.record_request(true, 10000, false, 0);

        let stats = metrics.get_stats();
        // P99 should be elevated due to the outlier (EMA tracking)
        assert!(
            stats.p99_latency_us > 100.0,
            "P99: {} (expected > 100)",
            stats.p99_latency_us
        );
    }
}

// =============================================================================
// Bonus: Debug Implementations
// =============================================================================

mod debug_implementations {
    use super::*;

    /// Verify MutableCircuitBreaker Debug impl
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn test_circuit_breaker_debug() {
        let cb = MutableCircuitBreaker::new(5, 60);
        let debug_str = format!("{:?}", cb);

        assert!(debug_str.contains("MutableCircuitBreaker"));
        assert!(debug_str.contains("state"));
        assert!(debug_str.contains("failure_threshold"));
    }

    /// Verify CircuitBreakerError Display impl
    #[test]
    #[cfg(feature = "client-circuit-breaker")]
    fn test_circuit_breaker_error_display() {
        let open_err = CircuitBreakerError::Open;
        let forced_err = CircuitBreakerError::ForcedOpen;

        assert!(format!("{}", open_err).contains("open"));
        assert!(format!("{}", forced_err).contains("forced"));
    }
}
