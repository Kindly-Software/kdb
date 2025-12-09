# Security Code Review Checklist - P1 Enhancements
**Security Expert Deliverable**

**Date**: 2025-10-21
**Framework**: ASSUM Safety + OWASP Top 10 + UCE34 Q34
**Purpose**: Pre-merge security validation for all P1/P2 enhancements
**Status**: ✅ READY FOR USE

---

## How to Use This Checklist

1. **Before Code Review**: Reviewer reads ASSUM_VALIDATION.md and P1_SECURITY_HARDENING.md
2. **During Review**: Check each item below, mark ✅ or ❌
3. **After Review**: All items must be ✅ before merge approval
4. **Blocking Issues**: Any ❌ in "CRITICAL" section blocks merge

---

## Part 1: ASSUM Framework Validation (CRITICAL)

### 1.1 Category 2: TYPE_SAFETY (unsafe code)

**For each `unsafe { }` block**:

- [ ] **#ASSUME_TYPE_SAFE tag present** with 4 invariants:
  - [ ] 1. Pointer validity (where allocated, lifetime)
  - [ ] 2. Alignment requirements (64B/128B/256B)
  - [ ] 3. Exclusive ownership or shared immutability
  - [ ] 4. No concurrent Drop while accessing

- [ ] **#VERIFY_UNSAFE_INVARIANTS tag present** with verification methods:
  - [ ] 1. Allocation method documented (Box::into_raw, etc.)
  - [ ] 2. Bounds checking before unsafe access
  - [ ] 3. Unit tests validate safety properties
  - [ ] 4. Optional: Miri validation passes

- [ ] **Bounds checking** before all pointer operations:
  - [ ] Index validated < capacity/len
  - [ ] Null pointer check in Drop impl
  - [ ] No arithmetic overflow in index calculation

**Example (Timeline Aggregation Capsule)**:
```rust
// ✅ GOOD: Bounds check before unsafe
if bucket_idx >= capacity {
    return Err(...);
}
let bucket = unsafe { self.get_bucket_unchecked(bucket_idx) };

// ❌ BAD: No bounds check
let bucket = unsafe { self.get_bucket_unchecked(user_input) };
```

---

### 1.2 Category 3: TOCTOU_PREVENTION (race conditions)

**For load-then-store patterns**:

- [ ] **CAS loop used** for atomic read-modify-write:
  - [ ] `compare_exchange` or `compare_exchange_weak`
  - [ ] Retry loop handles spurious failures
  - [ ] No naked `load()` then `store()` without CAS

- [ ] **#ASSUME_TOCTOU_SAFE tag present** with justification:
  - [ ] Explains how race is prevented (CAS/lock/single-writer)
  - [ ] Documents idempotency if CAS can spuriously fail

- [ ] **#VERIFY_TOCTOU_PREVENTED tag present**:
  - [ ] Loom model checking mentioned (if applicable)
  - [ ] Property tests with concurrent threads

**Example (Timeline Head Pointer)**:
```rust
// ✅ GOOD: CAS prevents TOCTOU
let current_head = self.head.load(Ordering::Relaxed);
if bucket_idx > current_head {
    self.head.compare_exchange_weak(
        current_head,
        bucket_idx,
        Ordering::Release,
        Ordering::Relaxed,
    );  // ✅ Atomic update
}

// ❌ BAD: TOCTOU race
let current_head = self.head.load(Ordering::Relaxed);
if bucket_idx > current_head {
    self.head.store(bucket_idx, Ordering::Release);  // ❌ Race!
}
```

---

### 1.3 Category 4: MEMORY_ORDERING (relaxed atomics)

**For each `Ordering::Relaxed`**:

- [ ] **#ASSUME_MEMORY_ORDERING tag present** with justification:
  - [ ] Explains why Relaxed is sufficient (statistics, no synchronization needed)
  - [ ] Documents what data does NOT need synchronization

- [ ] **#VERIFY_ORDERING_SUFFICIENT tag present** with measurement:
  - [ ] Benchmark: Relaxed vs Acquire/Release (e.g., "10ns Relaxed vs 20ns SeqCst")
  - [ ] Speedup documented (e.g., "2× faster")

**For synchronization operations (Acquire/Release/SeqCst)**:

- [ ] **Acquire** used for reads that synchronize with writes:
  - [ ] Reading shared state after CAS success
  - [ ] Reading hash after bucket flush

- [ ] **Release** used for writes that publish data:
  - [ ] CAS success path publishes bucket updates
  - [ ] Status update publishes state change
  - [ ] Hash store publishes bucket completion

- [ ] **SeqCst** used for complex multi-variable invariants:
  - [ ] Only when Acquire/Release insufficient
  - [ ] Documented why stronger ordering needed

**Example (Event Count)**:
```rust
// ✅ GOOD: Relaxed justified
// #ASSUME_MEMORY_ORDERING: Relaxed sufficient for statistics counter
// #VERIFY_ORDERING_SUFFICIENT: 10ns Relaxed vs 20ns SeqCst (2× faster)
self.event_count.fetch_add(1, Ordering::Relaxed);

// ✅ GOOD: Release for synchronization
self.hash.store(hash, Ordering::Release);  // Publish bucket completion
```

---

### 1.4 Category 7: METRIC_ATOMICITY (counters)

**For all metrics counters**:

- [ ] **`fetch_add` or `fetch_sub` used** (not `load`+`store`):
  - [ ] All increments are atomic operations
  - [ ] No manual `load()` then `store(val + 1)`

- [ ] **#ASSUME_METRIC_ATOMIC tag present**:
  - [ ] Documents all counters use atomics
  - [ ] States that no lost updates under contention

- [ ] **#VERIFY_COUNTER_ACCURACY tag present**:
  - [ ] Property test: 100 threads × 1000 increments = 100,000 total
  - [ ] No decrements observed (monotonicity)

**Example (Total Events)**:
```rust
// ✅ GOOD: Atomic increment
self.total_events.fetch_add(1, Ordering::Relaxed);

// ❌ BAD: Non-atomic increment
let current = self.total_events.load(Ordering::Relaxed);
self.total_events.store(current + 1, Ordering::Relaxed);  // ❌ Lost updates!
```

---

### 1.5 Category 9: INVARIANT_MAINTENANCE (bounds/validation)

**For all user inputs and bounds**:

- [ ] **Bounds checking** before array/slice access:
  - [ ] Index < capacity/len validated
  - [ ] Panic-free error returns (Result<T,E>)

- [ ] **#ASSUME_INVARIANT tag present** with invariant stated:
  - [ ] E.g., "Bucket index always < capacity"
  - [ ] E.g., "Percentile in range [0, 100]"

- [ ] **#VERIFY_INVARIANT tag present** with verification:
  - [ ] Runtime bounds check in all paths
  - [ ] Compile-time assertions (const assert)
  - [ ] Property tests validate invariant

**Example (Bucket Index)**:
```rust
// ✅ GOOD: Bounds check
// #ASSUME_INVARIANT: Bucket index always < capacity
// #VERIFY_INVARIANT: Runtime bounds check before unsafe access
if bucket_idx >= capacity {
    return Err(ClapiError::IoError("Index out of bounds".to_string()));
}
let bucket = unsafe { self.get_bucket_unchecked(bucket_idx) };

// ❌ BAD: No bounds check
let bucket = unsafe { self.get_bucket_unchecked(bucket_idx) };  // ❌ UB if out of bounds
```

---

### 1.6 Category 10: RESOURCE_CLEANUP (Drop)

**For each `impl Drop`**:

- [ ] **Null check** before deallocation:
  - [ ] `if !ptr.is_null()` before `Box::from_raw`
  - [ ] Prevents double-free

- [ ] **Correct size** for deallocation:
  - [ ] `Box::from_raw(slice::from_raw_parts_mut(ptr, capacity))`
  - [ ] Capacity matches original allocation

- [ ] **#ASSUME_RESOURCE_CLEANUP tag present**:
  - [ ] Documents Drop called exactly once per instance
  - [ ] States cleanup is safe even on partially initialized

- [ ] **#VERIFY_DROP_SAFE tag present**:
  - [ ] Valgrind leak check mentioned
  - [ ] ASAN no use-after-free
  - [ ] Unit tests for Drop edge cases

**Example (Timeline Bucket Array)**:
```rust
// ✅ GOOD: Safe Drop
impl Drop for TimelineAggregationCapsuleCore {
    fn drop(&mut self) {
        let ptr = self.bucket_ptr.load(Ordering::Relaxed) as *mut TimelineBucket;
        let capacity = self.capacity.load(Ordering::Relaxed) as usize;

        if !ptr.is_null() {  // ✅ Null check
            unsafe {
                let _ = Box::from_raw(std::slice::from_raw_parts_mut(ptr, capacity));
            }  // ✅ Correct size
        }
    }
}
```

---

## Part 2: Input Validation & Bounds Checking

### 2.1 E14 Builder Pattern

- [ ] **Duration validation**:
  - [ ] Lower bound: >= 1 second (prevents divide-by-zero)
  - [ ] Upper bound: <= 86400 seconds (prevents overflow)
  - [ ] Error returned (not panic) for invalid input

- [ ] **Adversarial input tests**:
  - [ ] Zero duration rejected
  - [ ] u64::MAX duration rejected
  - [ ] Sub-second (nanoseconds only) rejected

**Example**:
```rust
// ✅ GOOD: Bounds checking
if secs == 0 {
    return Err(ClapiError::InvalidRequest {
        reason: "Duration must be >= 1 second".to_string(),
    });
}
if secs > 86400 {
    return Err(ClapiError::InvalidRequest {
        reason: "Duration must be <= 1 day".to_string(),
    });
}
```

---

### 2.2 E15 Aggregation Helpers

- [ ] **Overflow protection** in `aggregate_sum()`:
  - [ ] Use `saturating_add` or `checked_add`
  - [ ] NOT plain `+` operator (can overflow)

- [ ] **Empty range handling**:
  - [ ] `aggregate_avg()` returns 0.0 for empty
  - [ ] `aggregate_max()` returns error for empty
  - [ ] `percentile()` returns 0 for empty

- [ ] **Percentile bounds**:
  - [ ] Upper bound: percentile <= 100
  - [ ] Index clamping: `idx.min(len - 1)`

- [ ] **NaN and infinity handling**:
  - [ ] `rate_of_change()` infinity documented
  - [ ] Division by zero prevented

**Example (Overflow Fix)**:
```rust
// ❌ BAD: Overflow risk
let total: u64 = snapshots.iter().map(|s| s.event_count).sum();

// ✅ GOOD: Saturating add
let total: u64 = snapshots
    .iter()
    .fold(0u64, |acc, s| acc.saturating_add(s.event_count));
```

---

### 2.3 SystemTime Validation

- [ ] **Epoch 0 rejection** (clock skew detection):
  - [ ] `if ts_secs == 0 { return Err(...) }`
  - [ ] Error message mentions "clock skew"

- [ ] **Before epoch handling**:
  - [ ] `duration_since(UNIX_EPOCH)` error handled
  - [ ] Structured error with context

**Example**:
```rust
// ✅ GOOD: Epoch 0 rejection
let ts_secs = timestamp
    .duration_since(UNIX_EPOCH)
    .map_err(|e| ClapiError::InvalidRequest {
        reason: format!("SystemTime before Unix epoch: {}", e),
    })?
    .as_secs();

if ts_secs == 0 {
    return Err(ClapiError::InvalidRequest {
        reason: "Rejecting epoch 0 (clock skew suspected)".to_string(),
    });
}
```

---

## Part 3: Q34 Auditability (COMPLIANCE)

### 3.1 Audit Trail Requirements

**For state-modifying operations**:

- [ ] **operator_id parameter** added to all state-changing methods:
  - [ ] `append_audited(timestamp, event_type, data, operator_id)`
  - [ ] `flush_audited(bucket_idx, operator_id)`
  - [ ] `rollback_audited(version, operator_id, justification)`

- [ ] **Hash chain integrity**:
  - [ ] Each audit entry has `prev_hash` field
  - [ ] `verify_audit_trail()` method validates chain
  - [ ] FNV-1a or stronger hash used

- [ ] **Timestamp precision**:
  - [ ] Microsecond precision (not seconds)
  - [ ] Monotonic timestamps (no clock skew)

**Compliance Mapping**:

- [ ] **SOX**: Operator ID recorded for all data modifications
- [ ] **SOC2**: Access controls logged
- [ ] **GDPR Art. 32**: Data modification logging
- [ ] **HIPAA**: Access logging with user identity

---

### 3.2 Audit Entry Capsule

- [ ] **ComputationalCapsule derive** applied:
  - [ ] `#[derive(ComputationalCapsule)]`
  - [ ] `#[capsule(alignment = 128, size = 128)]`
  - [ ] `#[repr(C, align(128))]`

- [ ] **Fields documented**:
  - [ ] `timestamp_us`: u64 (epoch microseconds)
  - [ ] `operation`: AtomicU8 (append/flush/query/rollback)
  - [ ] `operator_id`: AtomicU64 (who)
  - [ ] `bucket_idx`: AtomicU64 (what)
  - [ ] `param_hash`: AtomicU64 (how)
  - [ ] `prev_hash`: u64 (chain link)
  - [ ] `hash`: AtomicU64 (this entry hash)

---

## Part 4: Multi-Tenant Isolation (E24)

**When E24 is implemented**:

- [ ] **DashMap** used for tenant isolation:
  - [ ] Each tenant has separate timeline instance
  - [ ] No shared mutable state between tenants

- [ ] **Tenant ID validation**:
  - [ ] No injection attacks (SQL, command, path traversal)
  - [ ] Tenant ID used as map key (not in paths/queries)

- [ ] **Property tests**:
  - [ ] 100 tenants, concurrent access, no cross-contamination
  - [ ] Each tenant has exactly expected event count

**Example**:
```rust
// ✅ GOOD: Tenant isolation
pub struct MultiTenantTimeline {
    tenants: DashMap<String, TimelineAggregationCapsuleWrapper>,
}

impl MultiTenantTimeline {
    pub fn append(&self, tenant_id: &str, ...) -> ClapiResult<()> {
        let timeline = self.tenants
            .entry(tenant_id.to_string())
            .or_insert_with(|| TimelineAggregationCapsuleWrapper::default());

        timeline.append(...)  // ✅ Isolated per tenant
    }
}
```

---

## Part 5: Security Test Coverage

### 5.1 Required Test Cases

**Overflow and Boundary**:

- [ ] `test_percentile_invalid_upper_bound` (percentile > 100)
- [ ] `test_builder_zero_duration` (duration = 0)
- [ ] `test_builder_excessive_duration` (duration > 86400)
- [ ] `test_aggregate_sum_overflow` (saturating_add validation)

**Edge Cases**:

- [ ] `test_aggregate_avg_empty_range` (returns 0.0)
- [ ] `test_aggregate_max_empty_range` (returns error)
- [ ] `test_percentile_empty_range` (returns 0)

**NaN and Infinity**:

- [ ] `test_rate_of_change_infinity` (growth from 0)
- [ ] `test_rate_of_change_zero_to_zero` (no change)

**Concurrent Safety**:

- [ ] `test_concurrent_append_accuracy` (100 threads × 100 appends = 10,000)
- [ ] `test_concurrent_query_consistency` (all threads see same total)

**Input Validation**:

- [ ] `test_append_before_epoch` (epoch 0 rejected)
- [ ] `test_append_future_timestamp` (accepted)
- [ ] `test_query_range_inverted` (start > end returns empty)

**Audit Trail** (Q34):

- [ ] `test_bucket_hash_chain` (hashes are unique and chained)
- [ ] `test_bucket_status_transitions` (Active → Complete → Flushed)

**Property Tests**:

- [ ] `property_total_events_monotonic` (never decreases)
- [ ] `property_sum_equals_total` (sum of buckets = total counter)
- [ ] `property_percentile_within_range` (p50 in [min, max])

---

## Part 6: Documentation Requirements

### 6.1 ASSUM Tags

**Each assumption must have**:

- [ ] **#ASSUME_<CATEGORY> comment** above relevant code
- [ ] **#VERIFY_<CATEGORY> comment** with verification method
- [ ] **Explanation** of assumption (1-3 lines)
- [ ] **Evidence** of verification (test name, tool, measurement)

**Example**:
```rust
// #ASSUME_MEMORY_ORDERING: Relaxed sufficient for statistics counter
// #VERIFY_ORDERING_SUFFICIENT: 10ns Relaxed vs 20ns SeqCst (2× faster)
//   - Benchmark: cargo bench --bench timeline_ordering
//   - Result: 10.2ns Relaxed, 21.5ns SeqCst
self.event_count.fetch_add(1, Ordering::Relaxed);
```

---

### 6.2 Public API Documentation

**Each public method must document**:

- [ ] **Purpose** (what it does, 1 sentence)
- [ ] **Arguments** (with validation bounds)
- [ ] **Returns** (including error cases)
- [ ] **Performance** (target latency, e.g., "<100ns")
- [ ] **Safety** (if contains unsafe code)
- [ ] **Q34** (if audit trail operation)

**Example**:
```rust
/// Append event with audit trail (Q34 compliance)
///
/// # Arguments
/// - `timestamp`: Event timestamp (must be >= Unix epoch, epoch 0 rejected)
/// - `event_type`: Event type string (max 1KB)
/// - `data`: Event data string (max 1KB)
/// - `operator_id`: User or system ID performing append (Q34)
///
/// # Returns
/// - Ok(()) on success
/// - Err(ClapiError::InvalidRequest) if timestamp invalid
///
/// # Performance
/// - Target: <600ns with audit trail, <100ns without
///
/// # Q34 Compliance
/// - Records operator_id for SOX/SOC2/HIPAA
/// - Hash-chained audit entry for tamper detection
pub fn append_audited(
    &mut self,
    timestamp: SystemTime,
    event_type: &str,
    data: &str,
    operator_id: u64,
) -> ClapiResult<()>
```

---

## Part 7: Pre-Merge Approval

### 7.1 Blocking Issues (CRITICAL)

**Any ❌ in this section blocks merge**:

- [ ] All `unsafe` blocks have ASSUM tags
- [ ] All Relaxed orderings have justification
- [ ] All inputs are bounds-checked
- [ ] No TOCTOU races (CAS used)
- [ ] Drop impl has null check
- [ ] E15 overflow protection added (saturating_add)
- [ ] Epoch 0 rejection implemented
- [ ] All critical tests pass

---

### 7.2 High Priority (Should Fix Before Merge)

**Strongly recommended, but not blocking**:

- [ ] Property tests added for concurrent safety
- [ ] E24 multi-tenant isolation tests (if implemented)
- [ ] Q34 audit trail hash chain tests
- [ ] Miri validation passes (if unsafe code)
- [ ] Loom model checking passes (if complex concurrency)

---

### 7.3 Low Priority (Future Iteration)

**Can be addressed post-merge**:

- [ ] Comprehensive benchmarks (B32 validation)
- [ ] Stress tests (1M operations)
- [ ] Penetration testing (adversarial inputs)
- [ ] GDPR retention policy (audit trail)
- [ ] Secrets zeroization (if sensitive data)

---

## Part 8: Reviewer Sign-Off

**Reviewer**: _______________________
**Date**: _______________________

**ASSUM Rating**: _____ % safe

**Approval**: [ ] APPROVED [ ] CONDITIONAL [ ] REJECTED

**Conditions** (if conditional):
1. _______________________
2. _______________________
3. _______________________

**Signature**: _______________________

---

## Appendix: Quick Reference

### Common ASSUM Patterns

**Unsafe Code**:
```rust
// #ASSUME_TYPE_SAFE: Pointer valid, aligned, exclusive, no concurrent Drop
// #VERIFY_UNSAFE_INVARIANTS: Bounds check + unit tests + Miri
```

**TOCTOU Prevention**:
```rust
// #ASSUME_TOCTOU_SAFE: CAS loop prevents race
// #VERIFY_TOCTOU_PREVENTED: Property test with concurrent threads
```

**Relaxed Ordering**:
```rust
// #ASSUME_MEMORY_ORDERING: Relaxed sufficient for statistics
// #VERIFY_ORDERING_SUFFICIENT: 10ns Relaxed vs 20ns SeqCst (2× faster)
```

**Metrics**:
```rust
// #ASSUME_METRIC_ATOMIC: All increments are atomic
// #VERIFY_COUNTER_ACCURACY: 100 threads × 1000 = 100,000 total
```

**Invariants**:
```rust
// #ASSUME_INVARIANT: Index always < capacity
// #VERIFY_INVARIANT: Runtime bounds check in all paths
```

**Drop**:
```rust
// #ASSUME_RESOURCE_CLEANUP: Drop called exactly once, safe on partial init
// #VERIFY_DROP_SAFE: Valgrind clean, ASAN clean, null check
```

---

**Document Version**: 1.0
**Last Updated**: 2025-10-21
**Maintainer**: Security Expert
