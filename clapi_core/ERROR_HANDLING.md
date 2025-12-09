# Error Handling Guide - Clapi Core v0.2.0

**Version**: 0.2.0 (Pure Atomic Architecture)
**Date**: 2025-10-16
**Status**: Production-Ready

---

## Overview

Clapi Core v0.2.0 introduces **graceful error handling** with circuit breaker protection and lockfree retry mechanisms. All operations return `Result<T, ClapiError>` with zero panic paths.

### Design Principles

1. **Zero panics**: All operations return `Result`
2. **Graceful degradation**: Circuit breaker prevents cascading failures
3. **Client transparency**: Internal retries hide transient failures
4. **Rich context**: Errors include actionable information
5. **Monitoring-friendly**: Errors map to metrics

---

## Error Types

### 1. CircuitOpen

**When**: Circuit breaker opens due to high failure rate (>10%)

```rust
pub enum ClapiError {
    CircuitOpen {
        failure_rate: f64,    // Current failure rate (0.0-1.0)
        threshold: f64,       // Circuit open threshold (0.10 = 10%)
    },
    // ...
}
```

#### Causes
- Allocation failure rate >10% (e.g., CAS conflicts)
- Sustained slot exhaustion
- Memory pressure / allocation failures
- Corrupted internal state

#### Client Behavior
```rust
match budget_registry.try_deduct(budget_id, cost) {
    Err(ClapiError::CircuitOpen { failure_rate, threshold }) => {
        // Log error
        log::warn!("Circuit breaker open: {:.1}% failures (threshold: {:.1}%)",
                   failure_rate * 100.0, threshold * 100.0);

        // Wait for cooldown (60 seconds default)
        tokio::time::sleep(Duration::from_secs(60)).await;

        // Retry operation
        budget_registry.try_deduct(budget_id, cost)
    }
    Ok(new_budget) => { /* success */ }
    Err(e) => { /* other errors */ }
}
```

#### HTTP Response
```json
{
  "error": {
    "type": "circuit_open",
    "message": "Budget system temporarily unavailable (15.2% failure rate)",
    "failure_rate": 0.152,
    "threshold": 0.10,
    "retry_after": 60
  }
}
```

**Status Code**: `503 Service Unavailable`

#### Monitoring
```promql
# Alert when circuit opens
circuit_breaker_state == 1

# Alert on sustained high failure rate
rate(circuit_breaker_failures[1m]) /
rate(circuit_breaker_total_requests[1m]) > 0.10
```

---

### 2. AllocationConflict

**When**: CAS conflict during slot allocation (internal retry, rare)

```rust
pub enum ClapiError {
    AllocationConflict {
        slot_id: usize,       // Conflicting slot ID
        retry_count: usize,   // Number of retries attempted
    },
    // ...
}
```

#### Causes
- Concurrent allocation to same slot
- CAS retry exhausted (>3 attempts)
- Extreme contention (rare)

#### Client Behavior

**Internal Retry** (transparent to clients):
```rust
// Internal implementation (in BudgetRegistry)
fn allocate_with_retry(&self, budget_id: u64, initial: i64) -> ClapiResult<usize> {
    for attempt in 0..3 {
        match self.try_allocate_slot(budget_id, initial) {
            Ok(slot_id) => return Ok(slot_id),
            Err(ClapiError::AllocationConflict { .. }) if attempt < 2 => {
                // Exponential backoff
                std::thread::yield_now();
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(ClapiError::AllocationConflict {
        slot_id: 0,
        retry_count: 3,
    })
}
```

**Client Code** (rare cases where retry fails):
```rust
match budget_registry.try_deduct(budget_id, cost) {
    Err(ClapiError::AllocationConflict { slot_id, retry_count }) => {
        // Log warning (rare)
        log::warn!("Allocation conflict on slot {} after {} retries",
                   slot_id, retry_count);

        // Retry with exponential backoff
        tokio::time::sleep(Duration::from_millis(100)).await;
        budget_registry.try_deduct(budget_id, cost)
    }
    Ok(new_budget) => { /* success */ }
    Err(e) => { /* other errors */ }
}
```

#### HTTP Response
```json
{
  "error": {
    "type": "allocation_conflict",
    "message": "Temporary allocation conflict, please retry",
    "slot_id": 12345,
    "retry_count": 3,
    "retry_after": 0.1
  }
}
```

**Status Code**: `503 Service Unavailable`

#### Monitoring
```promql
# Alert on high conflict rate (>1% is unusual)
rate(allocation_conflicts[1m]) /
rate(allocation_attempts[1m]) > 0.01
```

---

### 3. SlotsExhausted

**When**: All 1M budget slots allocated

```rust
pub enum ClapiError {
    SlotsExhausted {
        max: usize,           // Maximum slots (1,000,000)
        current: usize,       // Current slot count
    },
    // ...
}
```

#### Causes
- 1M+ concurrent budgets allocated
- Inactive budgets not deallocated
- Memory leak / slot leak
- Capacity planning insufficient

#### Client Behavior
```rust
match budget_registry.try_deduct(budget_id, cost) {
    Err(ClapiError::SlotsExhausted { max, current }) => {
        // Critical error - alert monitoring
        log::critical!("Budget slots exhausted: {}/{}", current, max);

        // Trigger cleanup job (async)
        tokio::spawn(async {
            cleanup_inactive_budgets().await;
        });

        // Return error to client (cannot retry immediately)
        return Err("Budget system at capacity, please contact support".into());
    }
    Ok(new_budget) => { /* success */ }
    Err(e) => { /* other errors */ }
}
```

#### HTTP Response
```json
{
  "error": {
    "type": "slots_exhausted",
    "message": "Budget system at capacity (1,000,000/1,000,000 slots)",
    "max_slots": 1000000,
    "current_slots": 1000000,
    "contact_support": true
  }
}
```

**Status Code**: `507 Insufficient Storage`

#### Monitoring
```promql
# Alert on high utilization (>80%)
budget_slots_active / budget_slots_max > 0.80

# Alert on capacity exhausted
budget_slots_active >= budget_slots_max
```

---

### 4. BudgetExhausted

**When**: Insufficient budget for requested operation

```rust
pub enum ClapiError {
    BudgetExhausted {
        required: i64,        // Required amount (cents)
        available: i64,       // Available amount (cents)
    },
    // ...
}
```

#### Causes
- Budget spent
- Expensive operation requested
- Budget not credited

#### Client Behavior
```rust
match budget_registry.try_deduct(budget_id, cost) {
    Err(ClapiError::BudgetExhausted { required, available }) => {
        // Log insufficient budget
        log::info!("Insufficient budget: need ${:.2}, have ${:.2}",
                   required as f64 / 100.0, available as f64 / 100.0);

        // Request budget increase (external system)
        request_budget_increase(budget_id, required - available).await?;

        // Retry after credit
        budget_registry.try_deduct(budget_id, cost)
    }
    Ok(new_budget) => { /* success */ }
    Err(e) => { /* other errors */ }
}
```

#### HTTP Response
```json
{
  "error": {
    "type": "insufficient_budget",
    "message": "Insufficient budget: need $50.00, have $10.00",
    "required_cents": 5000,
    "available_cents": 1000,
    "deficit_cents": 4000
  }
}
```

**Status Code**: `402 Payment Required`

#### Monitoring
```promql
# Alert on high budget exhaustion rate
rate(budget_exhausted_errors[5m]) > 100
```

---

## Graceful Degradation Strategy

### Circuit Breaker State Machine

```
                     Failure rate >10%
       ┌─────────────────────────────────┐
       │                                 │
       │                                 ▼
   ┌───────┐                        ┌────────┐
   │       │   Cooldown (60s)       │        │
   │ Closed│◄───────────────────────┤  Open  │
   │       │   Failure rate <5%     │        │
   └───────┘                        └────────┘
       ▲                                 │
       │                                 │
       └─────────────────────────────────┘
               Continue failures
```

**States**:
- **Closed**: Normal operation, all requests allowed
- **Open**: Degraded operation, new requests rejected
- **Cooldown**: Testing recovery, limited requests allowed

**Thresholds**:
- Open circuit: >10% failure rate
- Close circuit: <5% failure rate (after cooldown)
- Cooldown period: 60 seconds

### Implementation

```rust
pub struct CircuitBreakerCapsule {
    failure_count: AtomicU64,
    total_requests: AtomicU64,
    state: AtomicU8, // 0 = Closed, 1 = Open
}

impl CircuitBreakerCapsule {
    pub fn allows_operation(&self) -> bool {
        match self.state.load(Ordering::Relaxed) {
            0 => true,  // Closed - allow
            1 => false, // Open - reject
            _ => false, // Unknown - reject (safe)
        }
    }

    pub fn record_success(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Update state if in cooldown
        let failure_rate = self.failure_rate();
        if failure_rate < 0.05 {
            self.state.store(0, Ordering::Release); // Close circuit
        }
    }

    pub fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Check if circuit should open
        let failure_rate = self.failure_rate();
        if failure_rate > 0.10 {
            self.state.store(1, Ordering::Release); // Open circuit
        }
    }

    fn failure_rate(&self) -> f64 {
        let failures = self.failure_count.load(Ordering::Relaxed);
        let total = self.total_requests.load(Ordering::Relaxed);

        if total == 0 {
            0.0
        } else {
            failures as f64 / total as f64
        }
    }
}
```

---

## Retry Logic

### Internal Retry (Transparent)

**Used for**: `AllocationConflict` (CAS failures)

```rust
fn allocate_with_retry(&self, budget_id: u64, initial: i64) -> ClapiResult<usize> {
    const MAX_RETRIES: usize = 3;

    for attempt in 0..MAX_RETRIES {
        match self.try_allocate_slot(budget_id, initial) {
            Ok(slot_id) => return Ok(slot_id),
            Err(ClapiError::AllocationConflict { .. }) => {
                if attempt == MAX_RETRIES - 1 {
                    // Final retry failed
                    return Err(ClapiError::AllocationConflict {
                        slot_id: 0,
                        retry_count: MAX_RETRIES,
                    });
                }

                // Exponential backoff
                let delay_ms = 2_u64.pow(attempt as u32);
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
            Err(e) => return Err(e), // Other errors - fail fast
        }
    }

    unreachable!()
}
```

### Client Retry (External)

**Used for**: `CircuitOpen`, `SlotsExhausted` (transient failures)

```rust
async fn deduct_with_retry(
    registry: &BudgetRegistry,
    budget_id: u64,
    cost: i64,
    max_retries: usize,
) -> ClapiResult<i64> {
    for attempt in 0..max_retries {
        match registry.try_deduct(budget_id, cost) {
            Ok(new_budget) => return Ok(new_budget),

            Err(ClapiError::CircuitOpen { .. }) => {
                if attempt == max_retries - 1 {
                    return Err(ClapiError::CircuitOpen {
                        failure_rate: 0.15,
                        threshold: 0.10,
                    });
                }

                // Wait for circuit cooldown
                tokio::time::sleep(Duration::from_secs(60)).await;
            }

            Err(ClapiError::AllocationConflict { .. }) => {
                // Exponential backoff
                let delay_ms = 100 * 2_u64.pow(attempt as u32);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            Err(e) => return Err(e), // Fail fast for other errors
        }
    }

    Err(ClapiError::CircuitOpen {
        failure_rate: 0.15,
        threshold: 0.10,
    })
}
```

---

## Error Response Format

### JSON Structure

```json
{
  "error": {
    "type": "error_type",
    "message": "Human-readable error message",
    "details": {
      // Error-specific fields
    },
    "retry_after": 60,  // Optional: seconds to wait before retry
    "contact_support": false  // Optional: requires operator intervention
  }
}
```

### HTTP Status Codes

| Error | Status | Retry | Contact Support |
|-------|--------|-------|-----------------|
| `CircuitOpen` | 503 | Yes (60s) | No |
| `AllocationConflict` | 503 | Yes (100ms) | No |
| `SlotsExhausted` | 507 | No | Yes |
| `BudgetExhausted` | 402 | No | No |
| `InvalidSlotId` | 400 | No | No |
| `SlotNotAllocated` | 404 | No | No |
| `ConfigError` | 500 | No | Maybe |

---

## Monitoring Recommendations

### Metrics to Track

```promql
# Error rates by type
rate(clapi_errors_total{type="circuit_open"}[5m])
rate(clapi_errors_total{type="allocation_conflict"}[5m])
rate(clapi_errors_total{type="slots_exhausted"}[5m])
rate(clapi_errors_total{type="budget_exhausted"}[5m])

# Circuit breaker
circuit_breaker_state  # 0 = closed, 1 = open
circuit_breaker_failure_rate
circuit_breaker_trip_count

# Slot utilization
budget_slots_active / budget_slots_max

# Retry counts
histogram_quantile(0.99, retry_count_histogram)
```

### Alerting Thresholds

```yaml
# High priority
- alert: CircuitBreakerOpen
  expr: circuit_breaker_state == 1
  for: 5m
  annotations:
    summary: "Budget system circuit breaker open"

- alert: SlotsExhausted
  expr: budget_slots_active >= budget_slots_max
  annotations:
    summary: "Budget slots capacity reached"

# Medium priority
- alert: HighSlotUtilization
  expr: budget_slots_active / budget_slots_max > 0.80
  for: 10m
  annotations:
    summary: "Budget slots 80%+ utilized"

# Low priority
- alert: HighAllocationConflictRate
  expr: rate(allocation_conflicts[5m]) /
        rate(allocation_attempts[5m]) > 0.01
  for: 15m
  annotations:
    summary: "High allocation conflict rate"
```

---

## Troubleshooting

### Circuit Breaker Open

**Symptoms**: `CircuitOpen` errors

**Diagnosis**:
```bash
# Check circuit breaker status
curl http://localhost:8080/health | jq .circuit_breaker

# Check failure rate
curl http://localhost:8080/metrics | grep circuit_breaker_failure_rate
```

**Fixes**:
1. Wait 60 seconds for cooldown
2. Check logs for root cause
3. Reduce request rate
4. Verify system health (CPU, memory)

### High Allocation Conflicts

**Symptoms**: `AllocationConflict` errors >1%

**Diagnosis**:
```bash
# Check conflict rate
cargo test --test budget_registry_stress_tests -- --ignored

# Profile contention
cargo flamegraph --bench budget_slot_lockfree_bench
```

**Fixes**:
1. Reduce concurrent allocation rate
2. Implement request throttling
3. Scale horizontally (if possible)
4. Monitor CPU usage

### Slots Exhausted

**Symptoms**: `SlotsExhausted` errors

**Diagnosis**:
```bash
# Check slot utilization
curl http://localhost:8080/metrics | grep budget_slots

# List inactive budgets
curl http://localhost:8080/admin/budgets?inactive=true
```

**Fixes**:
1. Deallocate inactive budgets
2. Implement periodic cleanup job
3. Increase cleanup frequency
4. Contact support for capacity increase

---

## Best Practices

### 1. Error Handling

```rust
// ✅ Good: Handle all error cases
match operation() {
    Ok(result) => handle_success(result),
    Err(ClapiError::CircuitOpen { .. }) => retry_with_backoff(),
    Err(ClapiError::AllocationConflict { .. }) => retry_with_backoff(),
    Err(ClapiError::SlotsExhausted { .. }) => trigger_cleanup(),
    Err(ClapiError::BudgetExhausted { .. }) => request_budget(),
    Err(e) => log_and_alert(e),
}

// ❌ Bad: Unwrap errors
operation().unwrap(); // Panics on error
```

### 2. Retry Logic

```rust
// ✅ Good: Exponential backoff with max retries
async fn retry_with_backoff<T, E>(
    mut f: impl FnMut() -> Result<T, E>,
    max_retries: usize,
) -> Result<T, E> {
    for attempt in 0..max_retries {
        match f() {
            Ok(result) => return Ok(result),
            Err(e) if attempt == max_retries - 1 => return Err(e),
            Err(_) => {
                let delay_ms = 100 * 2_u64.pow(attempt as u32);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
    unreachable!()
}

// ❌ Bad: Infinite retry loop
loop {
    if let Ok(result) = operation() {
        return result;
    }
    // No backoff, no limit
}
```

### 3. Monitoring

```rust
// ✅ Good: Track error metrics
match operation() {
    Ok(result) => {
        metrics::increment_counter!("operations_success");
        result
    }
    Err(e) => {
        metrics::increment_counter!("operations_error", "type" => error_type(&e));
        Err(e)
    }
}

// ❌ Bad: Silent failures
operation().ok(); // Swallows errors
```

---

## Conclusion

Clapi Core v0.2.0 provides robust error handling with:

✅ **Graceful degradation**: Circuit breaker prevents cascades
✅ **Transparent retries**: Internal retry for transient failures
✅ **Rich context**: Errors include actionable information
✅ **Zero panics**: All operations return `Result`
✅ **Monitoring-friendly**: Errors map to metrics

**Recommendation**: Implement retry logic, monitor metrics, alert on failures.

---

**Date**: 2025-10-16
**Author**: Documentation Expert
**Framework**: UCE33 (Tier 1 Atomic), ASSUM Safety, B32 Benchmarking
