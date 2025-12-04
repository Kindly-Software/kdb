# atomic_mcp_server - Actual API Signatures

**Purpose**: Reference document for test writers to ensure correct API usage.

**Last Updated**: 2025-11-18

---

## Core Server API

### McpServerCapsule

```rust
// Constructor - REQUIRES debugger reference!
pub fn new(debugger: &'static DebuggerCapsule) -> Self

// Main request handler
pub fn handle_request(
    &self,
    json: &str,
    api_key: Option<&str>,
    client_ip: Option<&str>,
    debugger: &DebuggerCapsule,
) -> Result<String, String>

// Statistics
pub fn get_stats(&self) -> ServerStats
```

**CRITICAL**: `McpServerCapsule::new()` requires a `&'static DebuggerCapsule` parameter. Tests must create a static debugger instance.

---

## License Validation

### LicenseValidatorCapsule

```rust
// Constructor
pub const fn new() -> Self

// Set license
pub fn set_license(&self, license_key: &str, expiry_unix: u64)

// Validate (cached, <10ns)
pub fn validate(&self) -> bool

// Validate with specific key (~100ns)
pub fn validate_key(&self, license_key: &str) -> bool

// Statistics
pub fn get_stats(&self) -> LicenseStats
```

**Usage Pattern**:
```rust
let validator = LicenseValidatorCapsule::new();
validator.set_license("test-key", 2000000000);
assert!(validator.validate());
assert!(validator.validate_key("test-key"));
```

---

## Tool Execution

### ToolExecutorCapsule

```rust
// Constructor
pub const fn new() -> Self

// Begin execution (returns generation counter)
pub fn begin_execution(&self, tool_id: u64) -> Result<u64, &'static str>

// Complete execution
pub fn complete_execution(
    &self,
    generation: u64,
    result_hash: u64,
    result_size: u64,
) -> Result<(), &'static str>

// Fail execution
pub fn fail_execution(
    &self,
    generation: u64,
    error_code: u64,
) -> Result<(), &'static str>

// Get current state
pub fn get_state(&self) -> ExecutionState

// Statistics
pub fn get_stats(&self) -> ExecutionStats

// Reset state
pub fn reset(&self)
```

**Usage Pattern**:
```rust
let executor = ToolExecutorCapsule::new();
let gen = executor.begin_execution(1)?;
executor.complete_execution(gen, 12345, 1024)?;
```

---

## Tool Registry

### McpToolRegistryCapsule

```rust
// Register tool
pub fn register_tool(&self, name: &str, handler_id: u64) -> Result<u64, &'static str>

// Lookup tool
pub fn lookup(&self, name: &str) -> Option<ToolHandle>

// Statistics
pub fn get_stats(&self) -> RegistryStats

// Record call latency
pub fn record_call(&self, latency_ns: u64)
```

**Usage Pattern**:
```rust
let registry = McpToolRegistryCapsule::new();
registry.register_tool("debugger/attach", 1)?;
let handle = registry.lookup("debugger/attach").unwrap();
```

---

## Rate Limiting (Per-Client)

### PerClientRateLimiterCapsule

**IMPORTANT**: All methods require a `&Arc<DashMap<ClientId, ClientTokenBucket>>` parameter!

```rust
// Constructor
pub const fn new(
    rate_per_sec: u64,
    burst_capacity: u64,
    refill_interval_ms: u64,
) -> Self

// Check rate limit (requires buckets HashMap!)
pub fn check_rate_limit(
    &self,
    buckets: &Arc<DashMap<ClientId, ClientTokenBucket>>,
    client_id: ClientId,
    now_ms: u64,
    cost: u64,
) -> Result<RateLimitDecision, RateLimitError>

// Refill tokens for all clients
pub fn refill_tokens(
    &self,
    buckets: &Arc<DashMap<ClientId, ClientTokenBucket>>,
    now_ms: u64,
) -> Result<(), RateLimitError>

// Set custom rate for client
pub fn set_client_rate(
    &self,
    buckets: &Arc<DashMap<ClientId, ClientTokenBucket>>,
    client_id: ClientId,
    rate_per_sec: u64,
    burst_capacity: u64,
    now_ms: u64,
) -> Result<(), RateLimitError>

// Get client statistics
pub fn get_client_stats(
    &self,
    buckets: &Arc<DashMap<ClientId, ClientTokenBucket>>,
    client_id: ClientId,
) -> Result<Option<ClientBucketStats>, RateLimitError>

// Cleanup stale clients
pub fn cleanup_stale_clients(
    &self,
    buckets: &Arc<DashMap<ClientId, ClientTokenBucket>>,
    now_ms: u64,
    stale_after_ms: u64,
) -> Result<usize, RateLimitError>

// Get aggregate statistics
pub fn get_stats(&self) -> PerClientRateLimiterStats

// Update defaults
pub fn set_defaults(
    &self,
    rate_per_sec: u64,
    burst_capacity: u64,
) -> Result<(), RateLimitError>
```

**Usage Pattern**:
```rust
use dashmap::DashMap;
use std::sync::Arc;

let limiter = PerClientRateLimiterCapsule::new(100 << 16, 200 << 16, 100);
let buckets = Arc::new(DashMap::<ClientId, ClientTokenBucket>::new());

let decision = limiter.check_rate_limit(&buckets, client_id, now_ms, 1 << 16)?;
if decision.allowed {
    // Process request
}
```

---

## JSON-RPC Parsing

### JsonRpcCapsule

```rust
// Parse JSON-RPC request
pub fn parse_request(&self, json: &str) -> Result<JsonRpcRequest, &'static str>

// Format success response
pub fn format_response(&self, id: i64, result: serde_json::Value) -> Result<String, &'static str>

// Format error response
pub fn format_error(&self, id: i64, code: i32, message: String) -> Result<String, &'static str>
```

**NOTE**: JSON-RPC parsing is typically internal to `McpServerCapsule`. Tests should call `handle_request()` instead of parsing directly.

---

## Common Test API Mismatches (FIXED)

### Before (BROKEN):
```rust
// ❌ WRONG: McpServerCapsule::new() requires debugger!
let server = McpServerCapsule::new();

// ❌ WRONG: Missing buckets HashMap
let decision = limiter.check_rate_limit(client_id, now_ms, cost)?;
```

### After (CORRECT):
```rust
// ✅ CORRECT: Create static debugger first
use kdb::DebuggerCapsule;
static DEBUGGER: DebuggerCapsule = DebuggerCapsule::new();
let server = McpServerCapsule::new(&DEBUGGER);

// ✅ CORRECT: Pass buckets HashMap
let buckets = Arc::new(DashMap::new());
let decision = limiter.check_rate_limit(&buckets, client_id, now_ms, cost)?;
```

---

## Return Types Reference

### RateLimitDecision
```rust
pub struct RateLimitDecision {
    pub allowed: bool,
    pub tokens_remaining: u64,
    pub retry_after_ms: Option<u64>,
}
```

### RateLimitError
```rust
pub enum RateLimitError {
    ClientNotFound,
    InvalidConfig { reason: String },
    Internal(String),
}
```

### ExecutionState
```rust
pub enum ExecutionState {
    Idle = 0,
    Executing = 1,
    Completed = 2,
    Failed = 3,
}
```

### LicenseStats
```rust
pub struct LicenseStats {
    pub validation_count: u64,
    pub validation_success: u64,
    pub validation_failed: u64,
    pub is_valid: bool,
    pub expiry_unix: u64,
}
```

---

## Quota Tracking

### QuotaTrackerCapsule

```rust
// Constructor
pub const fn with_limits(
    requests_limit: u64,
    bytes_limit: u64,
    operations_limit: u64,
) -> Self

// Check and increment quota (combined operation)
pub fn check_and_increment(&self, bytes: u64) -> Result<(), &'static str>

// Get statistics
pub fn get_stats(&self) -> QuotaStats
```

**Usage Pattern**:
```rust
let quota = QuotaTrackerCapsule::with_limits(10_000, 100_000, 1_000_000);
quota.check_and_increment(1024)?; // Check and consume 1024 bytes
let stats = quota.get_stats();
```

**Return Types**:
```rust
pub struct QuotaStats {
    pub current_requests: u64,
    pub current_bytes: u64,
    pub current_operations: u64,
    pub limit_requests: u64,
    pub limit_bytes: u64,
    pub limit_operations: u64,
}
```

---

## Rate Limiting (Simple)

### RateLimiterCapsule

**NOTE**: This is the simple global rate limiter. For per-client rate limiting, use `PerClientRateLimiterCapsule`.

```rust
// Constructor
pub const fn new() -> Self

// Check rate limit
pub fn check(&self, cost: u64) -> Result<(), u64>

// Get statistics
pub fn get_stats(&self) -> RateLimiterStats
```

**Usage Pattern**:
```rust
let limiter = RateLimiterCapsule::new();
match limiter.check(1 << 16) { // 1 token in Q16.16
    Ok(()) => { /* Allowed */ },
    Err(wait_ns) => { /* Rate limited, wait {wait_ns} nanoseconds */ },
}
```

**IMPORTANT**: `check()` returns `Result<(), u64>` where:
- `Ok(())` = request allowed
- `Err(wait_ns)` = rate limited, wait `wait_ns` nanoseconds before retry

---

## Imports Required

```rust
// Core server
use atomic_mcp_server::{
    McpServerCapsule,
    LicenseValidatorCapsule,
    ToolExecutorCapsule,
    McpToolRegistryCapsule,
    PerClientRateLimiterCapsule,
    RateLimitDecision,
    RateLimitError,
    ClientId,
};

// Debugger (required for McpServerCapsule::new())
use kdb::DebuggerCapsule;

// DashMap for rate limiter
use dashmap::DashMap;
use std::sync::Arc;
```

---

**Status**: This document reflects actual API signatures as of 2025-11-18. All test code must match these signatures exactly.
