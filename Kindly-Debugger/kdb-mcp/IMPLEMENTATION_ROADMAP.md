# Atomic MCP Server Testing - Implementation Roadmap (95/100 → 100/100)

**Status**: Ready to implement
**Documentation**: See TESTING_GAP_ANALYSIS.md (788 lines) and TESTING_QUICK_REFERENCE.md (315 lines)

## Summary: 5-Phase Plan (10 Days)

| Phase | Task | Effort | Tests | Dependencies |
|-------|------|--------|-------|--------------|
| 1 | Fix 4 critical bugs | 1-2d | - | None (blocking all phases) |
| 2 | Write critical tests | 3-4d | 78 | Phase 1 complete |
| 3 | Write medium tests | 2d | 37 | Phase 2 complete |
| 4 | Add benchmarks | 1d | - | All phases |
| 5 | Compliance + FI | 1d | 10 | All phases |
| **TOTAL** | | **8-10d** | **135+** | |

---

## Phase 1: Fix Critical Bugs (1-2 Days) - HIGH PRIORITY

### Bug 1: quota_tracker.rs:134-137 - Month Calculation

**File**: `/home/samuel/Primitives/atomic_mcp_server/src/quota_tracker.rs`
**Line**: 134-137
**Current Code**:
```rust
fn get_unix_month(&self, unix_seconds: u64) -> u64 {
    unix_seconds / (86400 * 30)  // ❌ BROKEN
}
```

**Impact**: Monthly quotas reset at wrong time (Feb has 28-29 days, not 30)

**Fix**:
```rust
fn get_unix_month(&self, unix_seconds: u64) -> u64 {
    // Simple month approximation: Unix timestamp → month number
    let days_since_epoch = unix_seconds / 86400;
    let approx_year = days_since_epoch / 365;
    let day_in_year = days_since_epoch % 365;
    // Use day-of-year to estimate month (approximate)
    approx_year * 12 + day_in_year / 30  // Rough approximation
    // Better: use chrono crate or implement proper calendar math
}
```

**Verification**: Test `test_quota_reset_at_month_boundary()` should pass

**Effort**: 30 minutes

---

### Bug 2: tool_registry.rs:70 - Bounds Check Logic

**File**: `/home/samuel/Primitives/atomic_mcp_server/src/tool_registry.rs`
**Line**: 70
**Current Code**:
```rust
if name.len() >= TOOL_NAME_LEN {  // ❌ WRONG: allows 64-byte names
    return Err("Tool name too long");
}
// ...
unsafe {
    core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), dest, name_bytes.len());
    // If name.len() == 64, this copies 64 bytes to 64-byte array with no room for null!
}
```

**Impact**: Potential buffer overflow if name is exactly 64 bytes

**Fix**:
```rust
if name.len() >= TOOL_NAME_LEN {  // ✅ Keep >= but ensure null-terminated
    return Err("Tool name too long");
}
// OR
if name.len() > (TOOL_NAME_LEN - 1) {  // ✅ Safer: require 1 byte for null
    return Err("Tool name too long");
}
```

**Verification**: Test `test_register_tool_overlong_name()` should verify rejection

**Effort**: 15 minutes

---

### Bug 3: http_transport.rs:73-75 - Hardcoded Stub Response

**File**: `/home/samuel/Primitives/atomic_mcp_server/src/http_transport.rs`
**Line**: 59-100, specifically 73-75
**Current Code**:
```rust
pub fn handle_rpc(&self, body: &str) -> Result<String, String> {
    // 5-step pipeline
    // ...
    let response = format!(r#"{{"jsonrpc":"2.0","id":1,"result":"ok","message":"Received HTTP request"}}"#);
    // ❌ HARDCODED response - never actually executes tool!
    // ...
}
```

**Impact**: HTTP requests don't execute tools, always return stub response

**Fix**:
```rust
pub fn handle_rpc(&self, body: &str) -> Result<String, String> {
    // Parse JSON-RPC request
    let req = self.json_rpc.parse_request(body)
        .map_err(|e| format!("Parse error: {}", e))?;
    
    // Execute via server
    let result = self.server.process_request(&req)
        .map_err(|e| format!("Execution error: {}", e))?;
    
    // Format response
    let response = self.json_rpc.format_response(req.id, result)
        .map_err(|e| format!("Format error: {}", e))?;
    
    Ok(response)
}
```

**Verification**: Test `test_http_handle_rpc_valid_request()` should execute actual tool

**Effort**: 1 hour

---

### Bug 4: stdio_transport.rs:64-68 - UnsafeCell Concurrent Access

**File**: `/home/samuel/Primitives/atomic_mcp_server/src/stdio_transport.rs`
**Line**: 64-68
**Current Code**:
```rust
pub input_buffer: UnsafeCell<[u8; 2048]>,
pub output_buffer: UnsafeCell<[u8; 2048]>,
// ❌ DANGER: Concurrent reads/writes without synchronization
```

**Impact**: Data races possible if concurrent threads access buffers

**Audit Required**: 
1. Who calls `read_line()` and `write_line()` on these buffers?
2. Are calls serialized (single-threaded), or concurrent?
3. If concurrent: add synchronization (Mutex around operations)
4. If single-threaded: add comment documenting thread-safety assumptions

**Fix Options**:
- Option A: If single-threaded → add `#[allow(unsafe_code)]` with comment explaining safety
- Option B: If concurrent → wrap in `Mutex<StdioTransportCapsule>` at usage site
- Option C: Use interior mutability patterns (RefCell with runtime checks)

**Verification**: Test `test_stdio_concurrent_read_write()` and code review

**Effort**: 2-3 hours (requires audit of call sites)

---

## Phase 2: Write Critical Tests (3-4 Days) - 78 Tests

### Test File 1: tests/server_tests.rs (25 tests, 2 days)

**File**: `/home/samuel/Primitives/atomic_mcp_server/tests/server_tests.rs`
**LOC**: ~800 lines

**Test Categories**:

#### Q1-Q7: Unit Tests (8 tests)
```rust
#[test] fn test_server_creation()
#[test] fn test_server_register_all_tools()
#[test] fn test_server_tool_lookup()
#[test] fn test_server_total_requests_incremented()
#[test] fn test_server_successful_requests_incremented()
#[test] fn test_server_failed_requests_incremented()
#[test] fn test_server_latency_histogram_recording()
#[test] fn test_server_audit_log_recording()
```

#### Q8-Q14: Property Tests (4 tests)
```rust
#[test] fn test_server_concurrent_requests()
#[test] fn test_server_metric_consistency()
#[test] fn test_server_audit_log_ordering()
#[test] fn test_server_histogram_correctness()
```

#### Q15-Q21: Integration Tests (9 tests)
```rust
#[test] fn test_server_process_request_success()
#[test] fn test_server_process_request_rate_limited()
#[test] fn test_server_process_request_quota_exceeded()
#[test] fn test_server_process_request_invalid_tool()
#[test] fn test_server_process_request_license_invalid()
#[test] fn test_server_avg_latency_calculation()
#[test] fn test_server_max_latency_tracking()
#[test] fn test_server_audit_log_ring_buffer_wraparound()
#[test] fn test_server_request_ordering()
```

#### Q22-Q28: Production Tests (4 tests)
```rust
#[test] fn test_server_stress_1m_requests()
#[test] fn test_server_stress_1000_concurrent()
#[test] fn test_server_all_12_tools_simultaneously()
#[test] fn test_server_latency_sla_validation()
```

**Key Dependencies**:
- Requires `server.process_request()` method to be exposed or testable
- Requires mocking/stubbing debugger attachment if needed

---

### Test File 2: tests/json_rpc_tests.rs (15 tests, 1 day)

**File**: `/home/samuel/Primitives/atomic_mcp_server/tests/json_rpc_tests.rs`
**LOC**: ~500 lines

#### Q1-Q7: Unit Tests (7 tests)
```rust
#[test] fn test_parse_valid_jsonrpc_request()
#[test] fn test_parse_invalid_jsonrpc_version()
#[test] fn test_parse_malformed_json()
#[test] fn test_parse_missing_required_fields()
#[test] fn test_format_response_valid()
#[test] fn test_format_response_with_error()
#[test] fn test_format_response_large_result()
```

#### Q8-Q14: Property Tests (4 tests)
```rust
#[test] fn test_parse_large_json_document()
#[test] fn test_parse_deeply_nested_json()
#[test] fn test_concurrent_parse_requests()
#[test] fn test_metrics_consistency()
```

#### Q15-Q21: Integration Tests (3 tests)
```rust
#[test] fn test_parse_then_format_roundtrip()
#[test] fn test_metrics_requests_parsed_increment()
#[test] fn test_metrics_bytes_in_out_tracking()
```

#### Q22-Q28: Production Tests (1 test)
```rust
#[test] fn test_json_rpc_stress_100k_requests()
```

---

### Test File 3: tests/tool_registry_tests.rs (18 tests, 1.5 days)

**File**: `/home/samuel/Primitives/atomic_mcp_server/tests/tool_registry_tests.rs`
**LOC**: ~600 lines

#### Q1-Q7: Unit Tests (7 tests)
```rust
#[test] fn test_register_tool_basic()
#[test] fn test_register_duplicate_tool()
#[test] fn test_register_64_tools_full_capacity()
#[test] fn test_register_tool_empty_name()
#[test] fn test_register_tool_max_length_name()
#[test] fn test_register_tool_overlong_name_rejects()  // Tests Bug 2 fix
#[test] fn test_lookup_basic()
```

#### Q8-Q14: Property Tests (5 tests)
```rust
#[test] fn test_lookup_nonexistent()
#[test] fn test_concurrent_register_same_slot()
#[test] fn test_concurrent_lookup_during_register()
#[test] fn test_register_with_embedded_nulls()
#[test] fn test_lookup_case_sensitive()
```

#### Q15-Q21: Integration Tests (4 tests)
```rust
#[test] fn test_record_call_increments_metrics()
#[test] fn test_lookup_handle_ptr_validity()  // Tests unsafe pointer safety
#[test] fn test_registry_stats_accuracy()
#[test] fn test_name_with_special_characters()
```

#### Q22-Q28: Production Tests (2 tests)
```rust
#[test] fn test_register_stress_capacity_exhaustion()
#[test] fn test_lookup_stress_high_contention()
```

---

### Test File 4: tests/tool_executor_tests.rs (20 tests, 1.5 days)

**File**: `/home/samuel/Primitives/atomic_mcp_server/tests/tool_executor_tests.rs`
**LOC**: ~650 lines

#### Q1-Q7: Unit Tests (8 tests)
```rust
#[test] fn test_execution_state_from_u8_valid()
#[test] fn test_execution_state_from_u8_invalid()
#[test] fn test_execution_state_to_u8_roundtrip()
#[test] fn test_metadata_new_creates_valid()
#[test] fn test_metadata_state_extraction()
#[test] fn test_metadata_tool_id_extraction()
#[test] fn test_metadata_generation_extraction()
#[test] fn test_executor_initial_state_idle()
```

#### Q8-Q14: Property Tests (6 tests)
```rust
#[test] fn test_metadata_with_state_transition()
#[test] fn test_executor_concurrent_dispatch()
#[test] fn test_executor_pending_count_tracking()
#[test] fn test_metadata_bit_packing_integrity()
#[test] fn test_executor_generation_counter()
#[test] fn test_executor_timeout_handling()
```

#### Q15-Q21: Integration Tests (4 tests)
```rust
#[test] fn test_executor_dispatch_transitions_executing()
#[test] fn test_executor_mark_completed()
#[test] fn test_executor_mark_failed()
#[test] fn test_executor_reset_clears_state()
```

#### Q22-Q28: Production Tests (2 tests)
```rust
#[test] fn test_executor_stress_rapid_dispatch()
#[test] fn test_executor_max_concurrent_tracking()
```

---

### Test File 5: tests/transport_integration_tests.rs (30 tests, 2 days)

**File**: `/home/samuel/Primitives/atomic_mcp_server/tests/transport_integration_tests.rs`
**LOC**: ~900 lines

#### HTTP Transport (12 tests)
```rust
#[test] fn test_http_handle_rpc_valid_request()
#[test] fn test_http_handle_rpc_malformed_body()
#[test] fn test_http_handle_rpc_response_format()
#[test] fn test_http_handle_rpc_large_request_body()
#[test] fn test_http_handle_rpc_empty_body()
#[test] fn test_http_transport_creation()
#[test] fn test_http_request_line_extraction()
#[test] fn test_http_response_buffering()
#[test] fn test_http_concurrent_requests()
#[test] fn test_http_buffer_wraparound()
#[test] fn test_http_metrics_tracking()
#[test] fn test_http_stress_high_throughput()
```

#### Stdio Transport (18 tests)
```rust
#[test] fn test_stdio_transport_creation()
#[test] fn test_stdio_read_complete_json_line()
#[test] fn test_stdio_read_incomplete_json()
#[test] fn test_stdio_read_multiple_json_lines()
#[test] fn test_stdio_read_empty_line()
#[test] fn test_stdio_read_very_long_line()
#[test] fn test_stdio_write_json_line()
#[test] fn test_stdio_write_batch_multiple()
#[test] fn test_stdio_buffer_overflow_handling()
#[test] fn test_stdio_invalid_json_in_line()
#[test] fn test_stdio_special_characters_in_json()
#[test] fn test_stdio_concurrent_read_write()  // Tests Bug 4 fix
#[test] fn test_stdio_read_metrics_tracking()
#[test] fn test_stdio_write_metrics_tracking()
#[test] fn test_stdio_bytes_tracking()
#[test] fn test_stdio_error_recovery()
#[test] fn test_stdio_buffer_wraparound()
#[test] fn test_stdio_stress_high_throughput()
```

---

## Phase 3: Write Medium-Priority Tests (2 Days) - 37 Tests

### Test File 6: tests/rate_limiter_tests.rs (10 tests, 1 day)

```rust
#[test] fn test_rate_limit_first_request_succeeds()
#[test] fn test_rate_limit_empty_cost_zero()
#[test] fn test_rate_limit_cost_exceeds_max()
#[test] fn test_rate_limit_concurrent_consume()
#[test] fn test_rate_limit_token_refill_after_wait()
#[test] fn test_rate_limit_refill_rate_zero()
#[test] fn test_rate_limit_max_tokens_cap()
#[test] fn test_rate_limit_stat_tracking()
#[test] fn test_rate_limit_concurrent_refill()
#[test] fn test_rate_limit_stress_high_contention()
```

### Test File 7: tests/quota_tracker_tests.rs (12 tests, 1 day)

```rust
#[test] fn test_quota_allow_within_daily_limit()
#[test] fn test_quota_daily_limit_exceeded()
#[test] fn test_quota_daily_reset_at_boundary()
#[test] fn test_quota_monthly_limit_exceeded()
#[test] fn test_quota_total_limit_exceeded()
#[test] fn test_quota_bytes_tracking()
#[test] fn test_quota_zero_bytes()
#[test] fn test_quota_large_bytes()
#[test] fn test_quota_concurrent_reset()
#[test] fn test_quota_reset_at_month_boundary()  // Tests Bug 1 fix
#[test] fn test_quota_tracking_accuracy()
#[test] fn test_quota_stress_concurrent_increments()
```

### Test File 8: tests/metrics_tests.rs (15 tests, 1 day)

```rust
#[test] fn test_tool_request_counter_new()
#[test] fn test_latency_histogram_record_10us_bucket()
#[test] fn test_latency_histogram_record_boundary_9999ns()
#[test] fn test_latency_histogram_record_boundary_10000ns()
#[test] fn test_latency_histogram_record_100ms_bucket()
#[test] fn test_latency_histogram_record_inf_bucket()
#[test] fn test_latency_histogram_sum_computation()
#[test] fn test_metrics_increment_success()
#[test] fn test_metrics_increment_error()
#[test] fn test_metrics_all_tools_counters()
#[test] fn test_metrics_concurrent_increments()
#[test] fn test_metrics_prometheus_format()
#[test] fn test_metrics_histogram_percentiles()
#[test] fn test_metrics_label_cardinality()
#[test] fn test_metrics_stress_100k_records()
```

---

## Phase 4: Add Benchmarks (1 Day) - 20 Hours

### Benchmark 1: benches/b32_json_rpc.rs (4 hours)
- Measure: parse_request() latency
- Measure: format_response() latency
- Vary: JSON size (small, medium, large)

### Benchmark 2: benches/b32_tool_registry.rs (4 hours)
- Measure: register_tool() latency
- Measure: lookup() latency
- Vary: registry load (1 tool, 32 tools, 64 tools)

### Benchmark 3: benches/b32_tool_executor.rs (4 hours)
- Measure: dispatch_tool() latency
- Measure: mark_completed() latency
- Vary: concurrent dispatch count

### Benchmark 4: benches/b32_rate_limiter.rs (4 hours)
- Measure: check() latency
- Measure: refill() latency
- Vary: token availability (empty, half-full, full)

### Benchmark 5: benches/b32_quota_tracker.rs (4 hours)
- Measure: check_and_increment() latency
- Measure: maybe_reset() latency
- Vary: quota state (under limit, at limit, exceeded)

---

## Phase 5: Q34 Compliance + Failure Injection (1 Day) - 10 Tests

### Q34 Compliance Tests (4 tests)
```rust
#[test] fn test_audit_trail_tamper_detection()
#[test] fn test_audit_trail_concurrent_append()
#[test] fn test_audit_trail_export_consistency()
#[test] fn test_audit_trail_signature_verification()
```

### Failure Injection Tests (6 tests)
```rust
#[test] fn test_http_transport_write_io_error()
#[test] fn test_stdio_transport_read_eof()
#[test] fn test_tool_executor_timeout()
#[test] fn test_rate_limiter_timestamp_overflow()
#[test] fn test_quota_reset_race_condition()
#[test] fn test_metrics_atomic_ordering_violation()
```

---

## Execution Checklist

### Day 1 (Bug Fixes + Planning)
- [ ] Fix quota_tracker.rs:134 (30 min)
- [ ] Fix tool_registry.rs:70 (15 min)
- [ ] Fix http_transport.rs:73 (1 hour)
- [ ] Audit stdio_transport.rs:64 (2-3 hours)
- [ ] Create test file structure (30 min)

### Days 2-4 (Critical Tests)
- [ ] server_tests.rs (25 tests, 2 days)
- [ ] json_rpc_tests.rs (15 tests, 1 day)
- [ ] tool_registry_tests.rs (18 tests, 1.5 days)
- [ ] tool_executor_tests.rs (20 tests, 1.5 days)
- [ ] transport_integration_tests.rs (30 tests, 2 days)

### Days 5-6 (Medium Tests)
- [ ] rate_limiter_tests.rs (10 tests, 1 day)
- [ ] quota_tracker_tests.rs (12 tests, 1 day)
- [ ] metrics_tests.rs (15 tests, 1 day)

### Day 7 (Benchmarks)
- [ ] b32_json_rpc.rs (4 hours)
- [ ] b32_tool_registry.rs (4 hours)
- [ ] b32_tool_executor.rs (4 hours)
- [ ] b32_rate_limiter.rs (4 hours)
- [ ] b32_quota_tracker.rs (4 hours)

### Day 8 (Compliance + FI)
- [ ] Q34 compliance tests (4 tests, 4 hours)
- [ ] Failure injection tests (6 tests, 4 hours)

### Day 9-10 (Validation + Polish)
- [ ] Run full test suite: `cargo test --lib --all-features`
- [ ] Fix any compilation errors
- [ ] Run benchmarks: `cargo bench --bench b32_*`
- [ ] Generate coverage report
- [ ] Final documentation

---

## Success Criteria

✅ **All 12 untested modules have dedicated test files**
✅ **All 4 critical bugs are fixed**
✅ **135+ new tests added**
✅ **T28 distribution: Q1-Q7 (35) + Q8-Q14 (40) + Q15-Q21 (35) + Q22-Q28 (25)**
✅ **5 new B32 benchmarks**
✅ **No test failures: `cargo test --lib --all-features` → "test result: ok"`
✅ **Integration tests prove end-to-end correctness**
✅ **Stress tests validate <10μs SLA latency**

**Final Result**: 623+ tests, 0 failures, **100/100 readiness**

