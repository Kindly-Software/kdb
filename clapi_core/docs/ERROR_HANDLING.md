# Error Handling & Logging Guide (P1 E18-E21)

**Version**: 1.0.0
**Date**: 2025-10-21
**Status**: Production Ready

---

## Table of Contents

1. [Error Classification Framework](#error-classification-framework)
2. [Retry Strategies](#retry-strategies)
3. [Alert Severity Mapping](#alert-severity-mapping)
4. [Structured Logging Setup](#structured-logging-setup)
5. [Common Error Scenarios](#common-error-scenarios)
6. [Integration Examples](#integration-examples)

---

## Error Classification Framework

All `ClapiError` variants are categorized into **5 error classes** for deterministic retry and alerting behavior:

### 1. TRANSIENT (Safe to Retry)

**Description**: Temporary failures due to external conditions (network, provider availability). Safe to retry with exponential backoff.

**Errors**:
- `Timeout { timeout_ms }`
- `ProviderError(String)`
- `ProviderUnhealthy { provider_id }`
- `RateLimitExceeded { quota, window_duration_secs }`
- `RateLimitExceededWithBackpressure { user_id, retry_after_ms, quota, throttle_rate_percent }`
- `RetryLimitExceeded { attempts }`
- `NoProvidersAvailable`
- `AllProvidersUnavailable`

**Retry Strategy**: Exponential backoff (100ms → 1.6s → 10s → 60s)

**Alert Severity**: Low to Medium

---

### 2. PERMANENT (Alert Required)

**Description**: Persistent failures requiring immediate intervention (resource exhaustion, data corruption). Do NOT retry without fixing root cause.

**Errors**:
- `BudgetExhausted { requested, available }`
- `SlotsExhausted { max, current }`
- `EpochFull`
- `HashChainCorrupted { entry_index }`
- `DatabaseError(String)`
- `QuotaExceeded { used, limit }`

**Retry Strategy**: Do NOT retry (alert ops team)

**Alert Severity**: High to Critical

---

### 3. CONFIGURATION (Code/Config Bugs)

**Description**: Invalid configuration or logic bugs in code. Require code fixes or config updates.

**Errors**:
- `InvalidCost(i64)`
- `InvalidProviderId(u16)`
- `InvalidSlotId { slot_id, max }`
- `SlotNotAllocated { slot_id }`
- `NoSlotsAllocated`
- `ConfigError(String)`
- `QueryError { message }`

**Retry Strategy**: Do NOT retry (fix code/config first)

**Alert Severity**: Low (not production if caught early)

---

### 4. USER_ERROR (Invalid Input)

**Description**: User provided invalid input (malformed JSON, wrong API key). Provide helpful guidance.

**Errors**:
- `InvalidRequest { reason }`
- `Unauthorized`
- `JsonError(String)`

**Retry Strategy**: Do NOT retry (user must fix input)

**Alert Severity**: Low (user education)

---

### 5. SECURITY (Audit Logging Required)

**Description**: Security-related errors (authentication failures, tampering detection). Always log to audit trail.

**Errors**:
- `Unauthorized`
- `HashChainCorrupted { entry_index }`

**Retry Strategy**: Do NOT retry (investigate immediately)

**Alert Severity**: Critical (for tampering), Medium (for auth failures)

---

## Retry Strategies

### Exponential Backoff Formula

```rust
use clapi_core::error::ClapiError;

fn retry_with_backoff(error: &ClapiError, attempt: u32) -> Option<Duration> {
    if !error.is_retryable() {
        return None; // Don't retry permanent/config/user errors
    }

    let base_backoff_ms = error.retry_backoff_ms()?;
    let backoff_ms = base_backoff_ms * 2_u64.pow(attempt);
    let max_backoff_ms = 60_000; // Cap at 60 seconds

    Some(Duration::from_millis(backoff_ms.min(max_backoff_ms)))
}
```

### Retry Decision Table

| Error Category | Retry? | Backoff | Max Retries | Example |
|----------------|--------|---------|-------------|---------|
| TRANSIENT | ✅ Yes | Exponential | 5 | `Timeout`, `ProviderError` |
| PERMANENT | ❌ No | N/A | 0 | `BudgetExhausted`, `HashChainCorrupted` |
| CONFIGURATION | ❌ No | N/A | 0 | `InvalidCost`, `ConfigError` |
| USER_ERROR | ❌ No | N/A | 0 | `InvalidRequest`, `Unauthorized` |
| SECURITY | ❌ No | N/A | 0 | `Unauthorized`, `HashChainCorrupted` |

### Retry Example

```rust
use clapi_core::error::ClapiError;
use std::time::Duration;
use tokio::time::sleep;

async fn retry_operation<F, T>(
    operation: F,
    max_retries: u32,
) -> Result<T, ClapiError>
where
    F: Fn() -> Result<T, ClapiError>,
{
    let mut attempt = 0;

    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(error) if error.is_retryable() && attempt < max_retries => {
                // Calculate exponential backoff
                if let Some(backoff_ms) = error.retry_backoff_ms() {
                    let delay = backoff_ms * 2_u64.pow(attempt);
                    sleep(Duration::from_millis(delay)).await;
                }

                attempt += 1;
            }
            Err(error) => return Err(error), // Don't retry
        }
    }
}
```

---

## Alert Severity Mapping

### Severity Levels

| Severity | Description | Response Time | Notification Channel |
|----------|-------------|---------------|----------------------|
| **Critical** | System degraded, immediate action | < 5 minutes | PagerDuty (phone call) |
| **High** | Service impact, alert ops team | < 30 minutes | PagerDuty (push notification) |
| **Medium** | Non-critical issue | < 4 hours | Slack (#alerts channel) |
| **Low** | Informational, no action needed | Next business day | Slack (#monitoring channel) |

### Error to Severity Mapping

```rust
use clapi_core::error::{ClapiError, AlertSeverity};

fn get_alert_severity(error: &ClapiError) -> AlertSeverity {
    error.alert_severity()
}

// Example mapping:
let error = ClapiError::HashChainCorrupted { entry_index: 42 };
assert_eq!(error.alert_severity(), AlertSeverity::Critical);

let error = ClapiError::Timeout { timeout_ms: 30000 };
assert_eq!(error.alert_severity(), AlertSeverity::Low);
```

### Alerting Integration

```rust
use clapi_core::error::{ClapiError, AlertSeverity};
use clapi_core::logging::LogContext;

async fn handle_error_with_alerting(error: &ClapiError) {
    // Log error with structured logging
    LogContext::new("request_handling")
        .with_error(error)
        .with_field("request_id", "req_12345")
        .log_error();

    // Alert based on severity
    match error.alert_severity() {
        AlertSeverity::Critical => {
            // Send PagerDuty alert (immediate phone call)
            pagerduty::trigger_incident(error).await;
        }
        AlertSeverity::High => {
            // Send PagerDuty alert (push notification)
            pagerduty::trigger_incident(error).await;
        }
        AlertSeverity::Medium => {
            // Send Slack alert
            slack::send_alert(error).await;
        }
        AlertSeverity::Low => {
            // Log only (no external alert)
        }
    }
}
```

---

## Structured Logging Setup

### Environment Configuration

Set `RUST_LOG` environment variable:

```bash
# Production (errors + warnings + info)
export RUST_LOG=info

# Development (includes debug events)
export RUST_LOG=debug

# Verbose (all events including performance traces)
export RUST_LOG=trace

# Module-specific logging
export RUST_LOG=clapi_core::proxy=trace,clapi_core::error=debug
```

### Initialize Tracing

```rust
use clapi_core::logging::init_tracing;

fn main() {
    // Production: JSON output for log aggregators
    init_tracing();

    // Development: Pretty-printed human-readable
    // init_tracing_pretty();

    // ... rest of application
}
```

### JSON Output Format

**Example log entry** (single line, formatted here for readability):

```json
{
  "timestamp": "2025-10-21T12:34:56.789Z",
  "level": "ERROR",
  "target": "clapi_core::proxy",
  "span": {
    "operation": "budget_check",
    "error": "Budget exhausted: requested 1000, available 500",
    "category": "Permanent",
    "severity": "High",
    "retryable": false,
    "timestamp_ns": 1729513696789000000
  },
  "fields": {
    "suggested_action": "Increase budget allocation or upgrade tier",
    "user_id": "user_123",
    "request_id": "req_456"
  },
  "message": "Operation failed: Budget exhausted: requested 1000, available 500"
}
```

### Logging Examples

#### 1. Error Logging

```rust
use clapi_core::logging::LogContext;
use clapi_core::error::ClapiError;

let error = ClapiError::BudgetExhausted {
    requested: 1000,
    available: 500,
};

LogContext::new("budget_check")
    .with_error(&error)
    .with_field("user_id", "user_123")
    .with_field("request_id", "req_456")
    .log_error();
```

#### 2. Performance Logging

```rust
use clapi_core::logging::PerformanceEvent;
use std::time::{Duration, Instant};

let start = Instant::now();
// ... perform operation
let elapsed = start.elapsed();

PerformanceEvent::new("budget_check")
    .with_latency(elapsed)
    .with_throughput(10_000_000) // 10M ops/s
    .with_metric("slots_allocated", 42)
    .log();
```

#### 3. Security Logging

```rust
use clapi_core::logging::SecurityEvent;

SecurityEvent::new("authentication_failed")
    .with_user_id("user_123")
    .with_ip_address("192.168.1.100")
    .with_reason("Invalid API key")
    .log();
```

#### 4. Lifecycle Logging

```rust
use clapi_core::logging::LifecycleEvent;

LifecycleEvent::new("worker_thread_started")
    .with_thread_id(42)
    .with_field("zone", "timeline_aggregation")
    .log();
```

---

## Common Error Scenarios

### Scenario 1: Provider Timeout (TRANSIENT)

**Error**: `Timeout { timeout_ms: 30000 }`

**Classification**: TRANSIENT

**Suggested Action**: "Retry request with exponential backoff (100ms → 1600ms)"

**Retry Strategy**:
```rust
let error = ClapiError::Timeout { timeout_ms: 30000 };
assert!(error.is_retryable());
assert_eq!(error.retry_backoff_ms(), Some(100)); // Start with 100ms

// Exponential backoff: 100ms → 200ms → 400ms → 800ms → 1600ms
```

---

### Scenario 2: Budget Exhausted (PERMANENT)

**Error**: `BudgetExhausted { requested: 1000, available: 500 }`

**Classification**: PERMANENT

**Suggested Action**: "Increase budget allocation or upgrade tier"

**Retry Strategy**:
```rust
let error = ClapiError::BudgetExhausted {
    requested: 1000,
    available: 500,
};
assert!(!error.is_retryable());
assert_eq!(error.alert_severity(), AlertSeverity::High);

// DO NOT retry - alert ops team
```

---

### Scenario 3: Hash Chain Tampering (SECURITY)

**Error**: `HashChainCorrupted { entry_index: 42 }`

**Classification**: SECURITY + PERMANENT

**Suggested Action**: "CRITICAL: Audit trail tampering detected, investigate immediately"

**Retry Strategy**:
```rust
let error = ClapiError::HashChainCorrupted { entry_index: 42 };
assert!(!error.is_retryable());
assert!(error.is_security_related());
assert_eq!(error.alert_severity(), AlertSeverity::Critical);

// DO NOT retry - immediate investigation required
// Log to security audit trail
```

---

### Scenario 4: Invalid JSON (USER_ERROR)

**Error**: `JsonError("expected value at line 1 column 1")`

**Classification**: USER_ERROR

**Suggested Action**: "Fix JSON syntax (malformed request body)"

**Retry Strategy**:
```rust
let error = ClapiError::JsonError("expected value at line 1 column 1".to_string());
assert!(!error.is_retryable());
assert!(error.is_user_error());
assert_eq!(error.alert_severity(), AlertSeverity::Low);

// DO NOT retry - user must fix JSON
// Return 400 Bad Request with helpful error message
```

---

### Scenario 5: Rate Limit Exceeded (TRANSIENT)

**Error**: `RateLimitExceeded { quota: 100, window_duration_secs: 60 }`

**Classification**: TRANSIENT

**Suggested Action**: "Reduce request rate or upgrade quota tier"

**Retry Strategy**:
```rust
let error = ClapiError::RateLimitExceeded {
    quota: 100,
    window_duration_secs: 60,
};
assert!(error.is_retryable());
assert_eq!(error.retry_backoff_ms(), Some(1000)); // 1 second backoff

// Retry after 1s (or wait for window to reset)
```

---

## Integration Examples

### Example 1: HTTP Handler with Error Logging

```rust
use axum::{http::StatusCode, Json};
use clapi_core::error::ClapiError;
use clapi_core::logging::LogContext;

async fn handle_request(request: Json<Request>) -> Result<Json<Response>, StatusCode> {
    match process_request(request).await {
        Ok(response) => Ok(Json(response)),
        Err(error) => {
            // Log error with context
            LogContext::new("handle_request")
                .with_error(&error)
                .with_field("request_id", "req_12345")
                .with_field("user_id", "user_123")
                .log_error();

            // Map error to HTTP status code
            let status = match error.category() {
                ErrorCategory::Transient => StatusCode::SERVICE_UNAVAILABLE,
                ErrorCategory::Permanent => StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCategory::Configuration => StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCategory::UserError => StatusCode::BAD_REQUEST,
                ErrorCategory::Security => StatusCode::UNAUTHORIZED,
            };

            Err(status)
        }
    }
}
```

---

### Example 2: Circuit Breaker with Error Classification

```rust
use clapi_core::error::ClapiError;
use clapi_core::logging::LogContext;

struct CircuitBreaker {
    // ... circuit breaker state
}

impl CircuitBreaker {
    async fn call_provider(&self, request: Request) -> Result<Response, ClapiError> {
        match self.make_request(request).await {
            Ok(response) => {
                // Success - close circuit breaker
                self.record_success();
                Ok(response)
            }
            Err(error) if error.is_retryable() => {
                // Transient error - increment failure counter
                self.record_failure();

                LogContext::new("circuit_breaker")
                    .with_error(&error)
                    .with_field("provider_id", "provider_1")
                    .log_error();

                Err(error)
            }
            Err(error) => {
                // Permanent/config/user error - don't affect circuit breaker
                LogContext::new("circuit_breaker")
                    .with_error(&error)
                    .log_error();

                Err(error)
            }
        }
    }
}
```

---

### Example 3: Audit Trail with Security Logging

```rust
use clapi_core::error::ClapiError;
use clapi_core::logging::{LogContext, SecurityEvent};

fn verify_hash_chain(entries: &[Entry]) -> Result<(), ClapiError> {
    let mut expected_hash = INITIAL_HASH;

    for (i, entry) in entries.iter().enumerate() {
        let stored_hash = entry.hash.load(Ordering::Acquire);

        if stored_hash != expected_hash {
            let error = ClapiError::HashChainCorrupted {
                entry_index: i as u64,
            };

            // Log security event
            SecurityEvent::new("hash_chain_tampering_detected")
                .with_reason(&format!("Entry {}: expected {}, got {}", i, expected_hash, stored_hash))
                .log();

            // Log error context
            LogContext::new("verify_hash_chain")
                .with_error(&error)
                .with_field("entry_index", i.to_string())
                .log_error();

            return Err(error);
        }

        expected_hash = stored_hash;
    }

    Ok(())
}
```

---

## Best Practices

### 1. Always Use Structured Logging

**Bad**:
```rust
println!("Error: {:?}", error); // Unstructured, no context
```

**Good**:
```rust
LogContext::new("operation_name")
    .with_error(&error)
    .with_field("context_key", "context_value")
    .log_error();
```

---

### 2. Classify Errors Correctly

**Bad**:
```rust
// Treating all errors as retryable
if error.is_some() {
    retry_operation();
}
```

**Good**:
```rust
if error.is_retryable() {
    retry_with_backoff(&error, attempt);
} else {
    alert_ops_team(&error);
}
```

---

### 3. Provide Helpful Error Messages

**Bad**:
```rust
ClapiError::ConfigError("Invalid config".to_string()) // Too vague
```

**Good**:
```rust
ClapiError::ConfigError(format!(
    "Invalid provider_id in config.toml: {} (must be 0-255)",
    provider_id
))
```

---

### 4. Log Security Events Separately

**Bad**:
```rust
LogContext::new("auth_check")
    .with_error(&error)
    .log_error(); // Lost in general error logs
```

**Good**:
```rust
// Log as security event for audit trail
SecurityEvent::new("authentication_failed")
    .with_user_id(&user_id)
    .with_ip_address(&ip_address)
    .with_reason("Invalid API key")
    .log();

// Also log as error for debugging
LogContext::new("auth_check")
    .with_error(&error)
    .log_error();
```

---

## Appendix: Error Classification Matrix

| Error Variant | Category | Retryable | Severity | Suggested Action |
|---------------|----------|-----------|----------|------------------|
| `BudgetExhausted` | PERMANENT | ❌ No | High | Increase budget allocation or upgrade tier |
| `InvalidCost` | CONFIGURATION | ❌ No | Low | Fix cost calculation logic |
| `NoProvidersAvailable` | TRANSIENT | ✅ Yes | High | Wait 60s for circuit breaker recovery |
| `ProviderUnhealthy` | TRANSIENT | ✅ Yes | Medium | Retry after cooldown (60s) |
| `HashChainCorrupted` | SECURITY | ❌ No | Critical | Investigate tampering immediately |
| `InvalidRequest` | USER_ERROR | ❌ No | Low | Check API documentation |
| `Timeout` | TRANSIENT | ✅ Yes | Low | Retry with exponential backoff |
| `RateLimitExceeded` | TRANSIENT | ✅ Yes | Medium | Reduce request rate or upgrade quota |
| `DatabaseError` | PERMANENT | ❌ No | Critical | Check database connection |
| `Unauthorized` | SECURITY | ❌ No | Medium | Provide valid API key |
| `SlotsExhausted` | PERMANENT | ❌ No | Critical | Increase slot capacity (1M slots) |
| `EpochFull` | PERMANENT | ❌ No | High | Flush epoch to disk and start new |
| `ConfigError` | CONFIGURATION | ❌ No | Low | Fix configuration file (TOML) |
| `JsonError` | USER_ERROR | ❌ No | Low | Fix JSON syntax |
| `QuotaExceeded` | PERMANENT | ❌ No | Medium | Upgrade plan or wait for reset |

---

**End of Error Handling & Logging Guide**
