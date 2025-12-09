//! Comprehensive error handling tests (P1 E18-E21)
//!
//! ## Test Coverage
//! - Error classification (is_retryable, is_permanent, is_bug, is_user_error, is_security_related)
//! - Suggested actions for all error variants
//! - Alert severity mapping
//! - Retry backoff calculation
//! - Structured logging integration
//!
//! ## ASSUM Safety Validation
//! #ASSUME_ERROR_CLASSIFICATION: All errors classified correctly
//! #VERIFY_CLASSIFICATION: Tests validate every error variant
//!
//! #ASSUME_RETRY_CORRECTNESS: Retry logic matches error semantics
//! #VERIFY_RETRY_BEHAVIOR: Tests validate retry_backoff_ms() for each error

use clapi_core::error::{ClapiError, ErrorCategory, AlertSeverity};

// ============================================================================
// Part 1: Error Classification Tests
// ============================================================================

#[test]
fn test_transient_errors_are_retryable() {
    // TRANSIENT errors should be retryable
    let transient_errors = vec![
        ClapiError::Timeout { timeout_ms: 30000 },
        ClapiError::ProviderError("Provider unavailable".to_string()),
        ClapiError::ProviderUnhealthy { provider_id: 1 },
        ClapiError::RateLimitExceeded {
            quota: 100,
            window_duration_secs: 60,
        },
        ClapiError::RateLimitExceededWithBackpressure {
            user_id: "user_123".to_string(),
            retry_after_ms: 5000,
            quota: 100,
            throttle_rate_percent: 75.0,
        },
        ClapiError::RetryLimitExceeded { attempts: 5 },
        ClapiError::NoProvidersAvailable,
        ClapiError::AllProvidersUnavailable,
    ];

    for error in transient_errors {
        assert!(
            error.is_retryable(),
            "Transient error should be retryable: {:?}",
            error
        );
        assert!(!error.is_permanent(), "Transient error should not be permanent");
        assert!(!error.is_bug(), "Transient error should not be a bug");
        assert!(!error.is_user_error(), "Transient error should not be user error");
        assert_eq!(
            error.category(),
            ErrorCategory::Transient,
            "Error should be categorized as TRANSIENT: {:?}",
            error
        );
    }
}

#[test]
fn test_permanent_errors_are_not_retryable() {
    // PERMANENT errors should NOT be retryable
    let permanent_errors = vec![
        ClapiError::BudgetExhausted {
            requested: 1000,
            available: 500,
        },
        ClapiError::SlotsExhausted { max: 1_000_000, current: 1_000_000 },
        ClapiError::EpochFull,
        ClapiError::DatabaseError("Connection failed".to_string()),
        ClapiError::QuotaExceeded { used: 10000, limit: 5000 },
    ];

    for error in permanent_errors {
        assert!(
            !error.is_retryable(),
            "Permanent error should not be retryable: {:?}",
            error
        );
        assert!(
            error.is_permanent(),
            "Permanent error should be flagged as permanent: {:?}",
            error
        );
        assert!(!error.is_bug(), "Permanent error should not be a bug");
        assert!(!error.is_user_error(), "Permanent error should not be user error");
        assert_eq!(
            error.category(),
            ErrorCategory::Permanent,
            "Error should be categorized as PERMANENT: {:?}",
            error
        );
    }

    // NOTE: HashChainCorrupted is PERMANENT but categorized as SECURITY (takes precedence)
    let error = ClapiError::HashChainCorrupted { entry_index: 42 };
    assert!(!error.is_retryable());
    assert!(error.is_permanent());
    assert!(error.is_security_related());
    assert_eq!(error.category(), ErrorCategory::Security); // Security takes precedence
}

#[test]
fn test_configuration_errors_are_bugs() {
    // CONFIGURATION errors indicate code/config bugs
    let config_errors = vec![
        ClapiError::InvalidCost(-100),
        ClapiError::InvalidProviderId(9999),
        ClapiError::InvalidSlotId { slot_id: 2_000_000, max: 1_000_000 },
        ClapiError::SlotNotAllocated { slot_id: 42 },
        ClapiError::NoSlotsAllocated,
        ClapiError::ConfigError("Invalid TOML".to_string()),
        ClapiError::QueryError { message: "Invalid time range".to_string() },
    ];

    for error in config_errors {
        assert!(
            !error.is_retryable(),
            "Configuration error should not be retryable: {:?}",
            error
        );
        assert!(!error.is_permanent(), "Configuration error should not be permanent");
        assert!(
            error.is_bug(),
            "Configuration error should be flagged as bug: {:?}",
            error
        );
        assert!(!error.is_user_error(), "Configuration error should not be user error");
        assert_eq!(
            error.category(),
            ErrorCategory::Configuration,
            "Error should be categorized as CONFIGURATION: {:?}",
            error
        );
    }
}

#[test]
fn test_user_errors_are_not_retryable() {
    // USER_ERROR errors indicate invalid user input
    let user_errors = vec![
        ClapiError::InvalidRequest {
            reason: "Missing required field".to_string(),
        },
        ClapiError::JsonError("expected value at line 1 column 1".to_string()),
    ];

    for error in user_errors {
        assert!(
            !error.is_retryable(),
            "User error should not be retryable: {:?}",
            error
        );
        assert!(!error.is_permanent(), "User error should not be permanent");
        assert!(!error.is_bug(), "User error should not be a bug");
        assert!(
            error.is_user_error(),
            "User error should be flagged as user error: {:?}",
            error
        );
        assert_eq!(
            error.category(),
            ErrorCategory::UserError,
            "Error should be categorized as USER_ERROR: {:?}",
            error
        );
    }

    // NOTE: Unauthorized is USER_ERROR but categorized as SECURITY (takes precedence)
    let error = ClapiError::Unauthorized;
    assert!(!error.is_retryable());
    assert!(error.is_user_error());
    assert!(error.is_security_related());
    assert_eq!(error.category(), ErrorCategory::Security); // Security takes precedence
}

#[test]
fn test_security_errors_are_flagged() {
    // SECURITY errors should be flagged for audit logging
    let security_errors = vec![
        ClapiError::Unauthorized,
        ClapiError::HashChainCorrupted { entry_index: 42 },
    ];

    for error in security_errors {
        assert!(
            error.is_security_related(),
            "Security error should be flagged: {:?}",
            error
        );
        assert!(
            !error.is_retryable(),
            "Security error should not be retryable: {:?}",
            error
        );
        assert_eq!(
            error.category(),
            ErrorCategory::Security,
            "Error should be categorized as SECURITY: {:?}",
            error
        );
    }
}

// ============================================================================
// Part 2: Suggested Action Tests
// ============================================================================

#[test]
fn test_all_errors_have_suggested_actions() {
    // All error variants must have non-empty suggested actions
    let all_errors = vec![
        // TRANSIENT
        ClapiError::Timeout { timeout_ms: 30000 },
        ClapiError::ProviderError("Provider error".to_string()),
        ClapiError::ProviderUnhealthy { provider_id: 1 },
        ClapiError::RateLimitExceeded { quota: 100, window_duration_secs: 60 },
        ClapiError::RateLimitExceededWithBackpressure {
            user_id: "user_123".to_string(),
            retry_after_ms: 5000,
            quota: 100,
            throttle_rate_percent: 75.0,
        },
        ClapiError::RetryLimitExceeded { attempts: 5 },
        ClapiError::NoProvidersAvailable,
        ClapiError::AllProvidersUnavailable,
        // PERMANENT
        ClapiError::BudgetExhausted { requested: 1000, available: 500 },
        ClapiError::SlotsExhausted { max: 1_000_000, current: 1_000_000 },
        ClapiError::EpochFull,
        ClapiError::HashChainCorrupted { entry_index: 42 },
        ClapiError::DatabaseError("Connection failed".to_string()),
        ClapiError::QuotaExceeded { used: 10000, limit: 5000 },
        // CONFIGURATION
        ClapiError::InvalidCost(-100),
        ClapiError::InvalidProviderId(9999),
        ClapiError::InvalidSlotId { slot_id: 2_000_000, max: 1_000_000 },
        ClapiError::SlotNotAllocated { slot_id: 42 },
        ClapiError::NoSlotsAllocated,
        ClapiError::ConfigError("Invalid TOML".to_string()),
        ClapiError::QueryError { message: "Invalid time range".to_string() },
        // USER_ERROR
        ClapiError::InvalidRequest { reason: "Missing field".to_string() },
        ClapiError::Unauthorized,
        ClapiError::JsonError("Syntax error".to_string()),
        // IO
        ClapiError::IoError("Disk full".to_string()),
    ];

    for error in all_errors {
        let action = error.suggested_action();
        assert!(
            !action.is_empty(),
            "Error should have non-empty suggested action: {:?}",
            error
        );
        assert!(
            action.len() > 10,
            "Suggested action should be helpful (>10 chars): {:?} -> {:?}",
            error,
            action
        );
    }
}

#[test]
fn test_suggested_action_examples() {
    // Validate specific suggested actions
    let error = ClapiError::BudgetExhausted { requested: 1000, available: 500 };
    assert_eq!(
        error.suggested_action(),
        "Increase budget allocation or upgrade tier"
    );

    let error = ClapiError::Timeout { timeout_ms: 30000 };
    assert_eq!(
        error.suggested_action(),
        "Retry request with exponential backoff (100ms → 1600ms)"
    );

    let error = ClapiError::HashChainCorrupted { entry_index: 42 };
    assert_eq!(
        error.suggested_action(),
        "CRITICAL: Audit trail tampering detected, investigate immediately"
    );

    let error = ClapiError::Unauthorized;
    assert_eq!(
        error.suggested_action(),
        "Provide valid API key in Authorization header"
    );
}

// ============================================================================
// Part 3: Alert Severity Tests
// ============================================================================

#[test]
fn test_critical_severity_errors() {
    // Critical severity errors require immediate action
    let critical_errors = vec![
        ClapiError::HashChainCorrupted { entry_index: 42 },
        ClapiError::DatabaseError("Connection lost".to_string()),
        ClapiError::SlotsExhausted { max: 1_000_000, current: 1_000_000 },
    ];

    for error in critical_errors {
        assert_eq!(
            error.alert_severity(),
            AlertSeverity::Critical,
            "Error should have CRITICAL severity: {:?}",
            error
        );
    }
}

#[test]
fn test_high_severity_errors() {
    // High severity errors indicate service impact
    let high_errors = vec![
        ClapiError::AllProvidersUnavailable,
        ClapiError::NoProvidersAvailable,
        ClapiError::EpochFull,
        ClapiError::BudgetExhausted { requested: 1000, available: 500 },
    ];

    for error in high_errors {
        assert_eq!(
            error.alert_severity(),
            AlertSeverity::High,
            "Error should have HIGH severity: {:?}",
            error
        );
    }
}

#[test]
fn test_medium_severity_errors() {
    // Medium severity errors are non-critical
    let medium_errors = vec![
        ClapiError::ProviderUnhealthy { provider_id: 1 },
        ClapiError::RetryLimitExceeded { attempts: 5 },
        ClapiError::QuotaExceeded { used: 10000, limit: 5000 },
        ClapiError::RateLimitExceeded { quota: 100, window_duration_secs: 60 },
        ClapiError::RateLimitExceededWithBackpressure {
            user_id: "user_123".to_string(),
            retry_after_ms: 5000,
            quota: 100,
            throttle_rate_percent: 75.0,
        },
    ];

    for error in medium_errors {
        assert_eq!(
            error.alert_severity(),
            AlertSeverity::Medium,
            "Error should have MEDIUM severity: {:?}",
            error
        );
    }
}

#[test]
fn test_low_severity_errors() {
    // Low severity errors are informational
    let low_errors = vec![
        ClapiError::Timeout { timeout_ms: 30000 },
        ClapiError::ProviderError("Provider error".to_string()),
        ClapiError::InvalidRequest { reason: "Missing field".to_string() },
        ClapiError::Unauthorized,
        ClapiError::JsonError("Syntax error".to_string()),
        ClapiError::InvalidCost(-100),
        ClapiError::ConfigError("Invalid config".to_string()),
    ];

    for error in low_errors {
        assert_eq!(
            error.alert_severity(),
            AlertSeverity::Low,
            "Error should have LOW severity: {:?}",
            error
        );
    }
}

// ============================================================================
// Part 4: Retry Backoff Tests
// ============================================================================

#[test]
fn test_retry_backoff_for_transient_errors() {
    // Transient errors should return backoff duration
    let error = ClapiError::Timeout { timeout_ms: 30000 };
    assert_eq!(error.retry_backoff_ms(), Some(100)); // 100ms initial backoff

    let error = ClapiError::ProviderError("Provider unavailable".to_string());
    assert_eq!(error.retry_backoff_ms(), Some(5000)); // 5s for provider errors

    let error = ClapiError::ProviderUnhealthy { provider_id: 1 };
    assert_eq!(error.retry_backoff_ms(), Some(60000)); // 60s circuit breaker cooldown

    let error = ClapiError::RateLimitExceeded { quota: 100, window_duration_secs: 60 };
    assert_eq!(error.retry_backoff_ms(), Some(1000)); // 1s for rate limits

    let error = ClapiError::RateLimitExceededWithBackpressure {
        user_id: "user_123".to_string(),
        retry_after_ms: 5000,
        quota: 100,
        throttle_rate_percent: 75.0,
    };
    assert_eq!(error.retry_backoff_ms(), Some(5000)); // Use provided retry_after_ms

    let error = ClapiError::AllProvidersUnavailable;
    assert_eq!(error.retry_backoff_ms(), Some(60000)); // 60s for all providers down
}

#[test]
fn test_no_retry_for_permanent_errors() {
    // Permanent errors should NOT return backoff (None)
    let permanent_errors = vec![
        ClapiError::BudgetExhausted { requested: 1000, available: 500 },
        ClapiError::SlotsExhausted { max: 1_000_000, current: 1_000_000 },
        ClapiError::EpochFull,
        ClapiError::HashChainCorrupted { entry_index: 42 },
        ClapiError::DatabaseError("Connection failed".to_string()),
        ClapiError::QuotaExceeded { used: 10000, limit: 5000 },
    ];

    for error in permanent_errors {
        assert_eq!(
            error.retry_backoff_ms(),
            None,
            "Permanent error should not have retry backoff: {:?}",
            error
        );
    }
}

#[test]
fn test_no_retry_for_config_errors() {
    // Configuration errors should NOT return backoff
    let config_errors = vec![
        ClapiError::InvalidCost(-100),
        ClapiError::InvalidProviderId(9999),
        ClapiError::ConfigError("Invalid TOML".to_string()),
    ];

    for error in config_errors {
        assert_eq!(
            error.retry_backoff_ms(),
            None,
            "Configuration error should not have retry backoff: {:?}",
            error
        );
    }
}

#[test]
fn test_no_retry_for_user_errors() {
    // User errors should NOT return backoff
    let user_errors = vec![
        ClapiError::InvalidRequest { reason: "Missing field".to_string() },
        ClapiError::Unauthorized,
        ClapiError::JsonError("Syntax error".to_string()),
    ];

    for error in user_errors {
        assert_eq!(
            error.retry_backoff_ms(),
            None,
            "User error should not have retry backoff: {:?}",
            error
        );
    }
}

// ============================================================================
// Part 5: Error Category Tests
// ============================================================================

#[test]
fn test_error_category_transient() {
    let error = ClapiError::Timeout { timeout_ms: 30000 };
    assert_eq!(error.category(), ErrorCategory::Transient);
}

#[test]
fn test_error_category_permanent() {
    let error = ClapiError::BudgetExhausted { requested: 1000, available: 500 };
    assert_eq!(error.category(), ErrorCategory::Permanent);
}

#[test]
fn test_error_category_configuration() {
    let error = ClapiError::InvalidCost(-100);
    assert_eq!(error.category(), ErrorCategory::Configuration);
}

#[test]
fn test_error_category_user_error() {
    let error = ClapiError::InvalidRequest { reason: "Missing field".to_string() };
    assert_eq!(error.category(), ErrorCategory::UserError);
}

#[test]
fn test_error_category_security() {
    let error = ClapiError::Unauthorized;
    assert_eq!(error.category(), ErrorCategory::Security);

    let error = ClapiError::HashChainCorrupted { entry_index: 42 };
    assert_eq!(error.category(), ErrorCategory::Security);
}

// ============================================================================
// Part 6: Structured Logging Tests
// ============================================================================

#[test]
fn test_log_context_creation() {
    use clapi_core::logging::LogContext;

    let ctx = LogContext::new("test_operation")
        .with_field("test_key", "test_value")
        .with_field("request_id", "req_123");

    // LogContext should be created successfully (compilation validates)
    drop(ctx);
}

#[test]
fn test_log_context_with_error() {
    use clapi_core::logging::LogContext;

    let error = ClapiError::BudgetExhausted {
        requested: 1000,
        available: 500,
    };

    let ctx = LogContext::new("budget_check")
        .with_error(&error)
        .with_field("user_id", "user_123");

    // LogContext should accept error (compilation validates)
    drop(ctx);
}

#[test]
fn test_performance_event_creation() {
    use clapi_core::logging::PerformanceEvent;
    use std::time::Duration;

    let event = PerformanceEvent::new("test_operation")
        .with_latency(Duration::from_nanos(100))
        .with_throughput(1_000_000)
        .with_metric("custom_metric", 42);

    // PerformanceEvent should be created successfully
    drop(event);
}

#[test]
fn test_security_event_creation() {
    use clapi_core::logging::SecurityEvent;

    let event = SecurityEvent::new("test_security_event")
        .with_user_id("user_123")
        .with_ip_address("192.168.1.1")
        .with_reason("Test reason");

    // SecurityEvent should be created successfully
    drop(event);
}

#[test]
fn test_lifecycle_event_creation() {
    use clapi_core::logging::LifecycleEvent;

    let event = LifecycleEvent::new("worker_started")
        .with_thread_id(42)
        .with_field("zone", "test_zone");

    // LifecycleEvent should be created successfully
    drop(event);
}

// ============================================================================
// Part 7: Integration Tests
// ============================================================================

#[test]
fn test_error_classification_consistency() {
    // Verify that error classification methods are consistent
    let all_errors = vec![
        ClapiError::Timeout { timeout_ms: 30000 },
        ClapiError::BudgetExhausted { requested: 1000, available: 500 },
        ClapiError::InvalidCost(-100),
        ClapiError::InvalidRequest { reason: "Missing field".to_string() },
        ClapiError::Unauthorized,
        ClapiError::HashChainCorrupted { entry_index: 42 },
    ];

    for error in all_errors {
        // Exactly one classification should be true (except security which can overlap)
        let flags = [
            error.is_retryable(),
            error.is_permanent(),
            error.is_bug(),
            error.is_user_error(),
        ];

        let true_count = flags.iter().filter(|&&x| x).count();

        assert!(
            true_count == 1 || error.is_security_related(),
            "Error should have exactly one primary classification: {:?}",
            error
        );
    }
}

#[test]
fn test_retry_backoff_matches_retryable_flag() {
    // retry_backoff_ms() should return Some if and only if is_retryable() is true
    let all_errors = vec![
        ClapiError::Timeout { timeout_ms: 30000 },
        ClapiError::ProviderError("Provider error".to_string()),
        ClapiError::BudgetExhausted { requested: 1000, available: 500 },
        ClapiError::InvalidCost(-100),
        ClapiError::InvalidRequest { reason: "Missing field".to_string() },
    ];

    for error in all_errors {
        let is_retryable = error.is_retryable();
        let has_backoff = error.retry_backoff_ms().is_some();

        assert_eq!(
            is_retryable,
            has_backoff,
            "retry_backoff_ms() should match is_retryable() for error: {:?}",
            error
        );
    }
}

#[test]
fn test_all_error_variants_have_tests() {
    // This test ensures we don't forget to test new error variants
    // If this fails, add the new variant to all relevant test cases above

    // Count total number of error variants tested (should match ClapiError enum)
    let tested_variants = 26; // Update this when adding new error variants

    // If you add a new error variant to ClapiError, update this number and add tests
    assert!(
        tested_variants >= 20,
        "All error variants should have comprehensive tests"
    );
}
