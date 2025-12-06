# Atomic MCP Server - Quick Fix Guide

**Priority**: URGENT - Build is currently broken
**Time Estimate**: 1-2 hours for critical fixes
**Validation**: cargo check will confirm

---

## CRITICAL FIX #1: Update Cargo.toml (1 min)

**File**: `/home/samuel/Primitives/atomic_mcp_server/Cargo.toml` line 16

**Change FROM**:
```toml
atomic_debugger = { version = "0.1", path = "../atomic_debugger", features = ["std", "simd"] }
```

**Change TO**:
```toml
kdb = { version = "0.1", path = "../kdb", features = ["std", "simd"] }
```

---

## CRITICAL FIX #2: Update server.rs Import (2 min)

**File**: `/home/samuel/Primitives/atomic_mcp_server/src/server.rs` line 17

**Change FROM**:
```rust
use atomic_debugger::DebuggerCapsule;
```

**Change TO**:
```rust
use kdb::DebuggerCapsule;
```

---

## CRITICAL FIX #3: Update lib.rs Module Declarations (10 min)

**File**: `/home/samuel/Primitives/atomic_mcp_server/src/lib.rs`

**ADD after line 48** (after `pub mod tools;`):
```rust
pub mod ab_testing;
pub mod access_control;
pub mod acme_cert_manager;
pub mod anomaly_detector;
pub mod api_key_auth;
pub mod audit_enhancement;
pub mod audit_log_rotation;
pub mod auth_guard;
pub mod auth_token;
pub mod capability_checker;
pub mod connection_pool;
pub mod dynamic_pid_whitelist;
pub mod feature_flags;
pub mod hsm_integration;
pub mod http_transport;
pub mod intrusion_detector;
pub mod key_rotation;
pub mod memory_encryption;
pub mod metrics;
pub mod per_client_rate_limiter;
pub mod runtime;
pub mod secrets_manager;
pub mod session;
pub mod shared_state;
pub mod stdio_transport;
pub mod tls_capsule;
pub mod tool_executor;
pub mod totp_validator;
pub mod tracing_setup;
pub mod zero_trust_policy;
```

**ADD after line 56** (after `pub use license_validator::LicenseValidatorCapsule;`):
```rust
pub use metrics::MetricsCapsule;
```

---

## CRITICAL FIX #4: Check Compilation (5 min)

Run this command to verify fixes:
```bash
cd /home/samuel/Primitives/atomic_mcp_server
cargo check 2>&1 | head -50
```

**Expected output after fixes**:
```
Checking atomic_mcp_server v0.1.0
Finished check [unoptimized + debuginfo] target(s) in 1.23s
```

**If you see errors about private modules**, check that all 31 modules are declared in lib.rs.

---

## MAJOR FIX #1: Fix HTTP Metrics Handler

**File**: `/home/samuel/Primitives/atomic_mcp_server/src/http_transport.rs` lines 152-165

**Change FROM**:
```rust
async fn metrics_handler() -> impl IntoResponse {
    // In a real implementation, this would be passed as state
    // For now, we create a temporary instance for demo
    let metrics = MetricsCapsule::new();
    let prometheus_output = metrics.export_prometheus();
    // ...
}
```

**Change TO** (Option 1 - using state, PREFERRED):
```rust
async fn metrics_handler(
    State(state): State<McpHttpServerState>,
) -> impl IntoResponse {
    // Use the metrics from server state (not create new)
    // This requires passing MetricsCapsule in McpHttpServerState
    let prometheus_output = state.server.get_metrics().export_prometheus();
    // ...
}
```

**Alternative** (Option 2 - using lazy_static):
```rust
use once_cell::sync::Lazy;

static GLOBAL_METRICS: Lazy<MetricsCapsule> = Lazy::new(MetricsCapsule::new);

async fn metrics_handler() -> impl IntoResponse {
    let prometheus_output = GLOBAL_METRICS.export_prometheus();
    // ...
}
```

---

## MAJOR FIX #2: Fix JsonRpcRequest id Field (Optional but Recommended)

**File**: `/home/samuel/Primitives/atomic_mcp_server/src/json_rpc.rs` line 176

**Change FROM**:
```rust
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}
```

**Change TO**:
```rust
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,  // Now accepts string or number IDs
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}
```

**Note**: This requires updating code that uses `id` field from `u64` to `Value`.

---

## VALIDATION STEPS

After applying fixes, run:

### Step 1: Check Compilation
```bash
cd /home/samuel/Primitives/atomic_mcp_server
cargo check
```

Expected: Should pass with no errors

### Step 2: Build Release
```bash
cargo build --release
```

Expected: Should create binary successfully

### Step 3: Run Tests (if available)
```bash
cargo test --lib
```

Expected: Tests should compile and run

### Step 4: Quick Validation
```bash
cargo run --example mcp_server_demo 2>&1 | head -20
```

Expected: Example should run without panics

---

## WHAT NOT TO FIX (Yet)

Don't change these yet - they're MAJOR but not blocking:
- [ ] ToolExecutorCapsule implementation (needs design decision)
- [ ] StdioTransportCapsule thread safety tests (needs test framework)
- [ ] JsonRpcRequest id field type (backwards compat impact - needs careful planning)
- [ ] Error handling consolidation (refactoring - can defer)
- [ ] Orphaned modules decision (30 modules - needs product decision: keep or remove?)

---

## AFTER CRITICAL FIXES

Once compilation succeeds:

1. **Re-run this validation**:
   ```bash
   grep -c "^pub mod\|^pub use" src/lib.rs
   # Should see ~60 (31 mods + 6 use + 25 doc items)
   ```

2. **Check for remaining import errors**:
   ```bash
   cargo check 2>&1 | grep "could not find" | head -10
   ```

3. **Run full test suite**:
   ```bash
   cargo test --all-features
   ```

4. **Check benchmarks compile**:
   ```bash
   cargo bench --no-run
   ```

---

## ROLLBACK PLAN

If things break after fixes:

1. Revert Cargo.toml: `git checkout Cargo.toml`
2. Revert server.rs: `git checkout src/server.rs`
3. Revert lib.rs: `git checkout src/lib.rs`
4. Start over with careful one-at-a-time changes

---

## SUCCESS CRITERIA

All three should pass:

```bash
✅ cargo check              # Compiles with no errors
✅ cargo build --release    # Creates release binary
✅ cargo test --lib         # Tests pass or skip cleanly
```

If all three pass, critical integration failures are fixed!

---

## ESTIMATED TIME

- **Critical Fixes (MUST DO)**: 15 minutes
- **Major Fixes (SHOULD DO)**: 30-45 minutes  
- **Validation (ALWAYS DO)**: 10 minutes
- **Total**: ~1 hour for working build

---

## SUPPORT

If you get stuck:
1. Check the full report: `INTEGRATION_VALIDATION_REPORT.md`
2. Look at specific issue with `grep -n "CRIT\|MAJ" INTEGRATION_VALIDATION_REPORT.md`
3. Review the files section for affected files
