# Troubleshooting Guide

Common errors, solutions, and debugging techniques for Clapi Core.

## Quick Diagnostics

### Check Server Health

```bash
curl http://localhost:8080/health
```

**Expected output** (healthy):
```json
{
  "status": "healthy",
  "providers": [
    {"id": "anthropic", "state": "Closed", "failure_rate_bp": 0}
  ]
}
```

### Check Logs

```bash
# Start server with debug logging
RUST_LOG=debug clapi --config clapi.toml

# Check specific component
RUST_LOG=clapi_core::proxy::budget_registry=trace clapi --config clapi.toml
```

### Verify Configuration

```bash
# Validate config without starting server
clapi --config clapi.toml --validate-only
```

## Common Errors

### 1. Budget Exhausted

**Error**:
```json
{
  "error": "BudgetExhausted",
  "required": 5000,
  "available": 0
}
```

**Cause**: Budget fully depleted.

**Solutions**:

**Option A: Check current budget**:
```bash
curl http://localhost:8080/metrics/budget
```

**Option B: Refill budget** (requires admin key):
```bash
curl -X POST http://localhost:8080/admin/budget/refill \
  -H "Authorization: Bearer ${ADMIN_API_KEY}" \
  -d '{
    "budget_id": "my_budget",
    "amount_cents": 10000
  }'
```

**Option C: Increase default budget**:
```toml
[server]
default_budget_cents = 500_00  # $500 initial budget
```

### 2. Circuit Breaker Open

**Error**:
```json
{
  "error": "CircuitOpen",
  "provider": "anthropic",
  "failure_rate_bp": 1500,
  "threshold_bp": 1000
}
```

**Cause**: Provider failing >10% (default threshold).

**Solutions**:

**Option A: Wait for cooldown** (default 60s):
```bash
# Check remaining cooldown time
curl http://localhost:8080/metrics/circuit_breaker
```

**Option B: Force circuit reset** (admin only):
```bash
curl -X POST http://localhost:8080/admin/circuit/reset \
  -H "Authorization: Bearer ${ADMIN_API_KEY}" \
  -d '{"provider_id": "anthropic"}'
```

**Option C: Adjust threshold**:
```toml
[circuit_breaker]
failure_threshold_bp = 2000  # Increase to 20%
cooldown_secs = 30            # Reduce cooldown
```

**Option D: Add fallback provider**:
```toml
[[providers]]
id = "backup"
priority = 2  # Used when primary circuit opens
```

### 3. Slots Exhausted

**Error**:
```json
{
  "error": "SlotsExhausted",
  "max": 1000000,
  "current": 1000000
}
```

**Cause**: Maximum concurrent budgets reached.

**Solutions**:

**Option A: Increase slot count**:
```toml
[server]
max_budget_slots = 10_000_000  # 10M slots (1.28GB RAM)
```

**Memory calculation**: `slots × 128 bytes`
- 1M slots = 128 MB
- 10M slots = 1.28 GB
- 100M slots = 12.8 GB

**Option B: Clean up inactive budgets** (admin only):
```bash
# Remove budgets inactive >30 days
curl -X POST http://localhost:8080/admin/budget/cleanup \
  -H "Authorization: Bearer ${ADMIN_API_KEY}" \
  -d '{"inactive_days": 30}'
```

### 4. Allocation Conflict (CAS Failure)

**Error**:
```json
{
  "error": "AllocationConflict",
  "slot_id": 12345,
  "retry_count": 3
}
```

**Cause**: High contention on budget slot (rare, <1% under normal load).

**Solutions**:

**Option A: Automatic retry** (built-in, 3 attempts):
- No action needed, retries happen automatically
- Error only if all 3 attempts fail

**Option B: Reduce contention**:
- Use unique budget IDs per user/team
- Avoid sharing budget IDs across many concurrent requests

**Option C: Check metrics**:
```bash
curl http://localhost:8080/metrics | jq '.budget.allocation.conflict_rate'
```

**Normal**: <1% conflict rate
**High**: >5% conflict rate (investigate load patterns)

### 5. Provider Timeout

**Error**:
```json
{
  "error": "ProviderTimeout",
  "provider": "anthropic",
  "timeout_secs": 60
}
```

**Cause**: Provider request exceeded timeout.

**Solutions**:

**Option A: Increase timeout**:
```toml
[[providers]]
id = "anthropic"
timeout_secs = 120  # 2 minutes
```

**Option B: Check provider status**:
```bash
curl https://status.anthropic.com
curl https://status.openai.com
```

**Option C: Enable retries**:
```toml
[[providers]]
id = "anthropic"
max_retries = 3  # Retry up to 3 times
```

### 6. Hash Chain Verification Failed

**Error**:
```
Error: Hash chain verification failed at index 1543
Expected: 0x1a2b3c4d5e6f7890
Actual:   0x9876543210fedcba
```

**Cause**: Audit log tampering or corruption.

**Solutions**:

**Option A: Restore from backup**:
```bash
cp /backup/audit_logs/2025-10-18.json ./audit_logs/
```

**Option B: Rebuild hash chain**:
```bash
curl -X POST http://localhost:8080/admin/audit/rebuild \
  -H "Authorization: Bearer ${ADMIN_API_KEY}"
```

**Option C: Disable validation** (NOT recommended for production):
```toml
[audit]
hash_chain_validation = false
```

### 7. Invalid API Key

**Error**:
```json
{
  "error": "Unauthorized",
  "provider": "anthropic",
  "status": 401
}
```

**Cause**: Invalid or expired provider API key.

**Solutions**:

**Option A: Update API key** (environment variable):
```bash
export CLAPI_PROVIDER_ANTHROPIC_API_KEY="sk-ant-new-key-..."
clapi --config clapi.toml
```

**Option B: Update API key** (config file):
```toml
[[providers]]
id = "anthropic"
api_key = "sk-ant-new-key-..."
```

**Option C: Verify key**:
```bash
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: sk-ant-..." \
  -H "anthropic-version: 2023-06-01" \
  -d '{"model":"claude-3-5-sonnet-20241022","messages":[{"role":"user","content":"test"}],"max_tokens":10}'
```

### 8. Configuration Validation Failed

**Error**:
```
Error: Invalid configuration at line 15
  failure_threshold_bp must be in range [0, 10000] (got 15000)
```

**Cause**: Invalid configuration value.

**Solutions**:

**Option A: Fix value**:
```toml
[circuit_breaker]
failure_threshold_bp = 1000  # 10% (not 15000)
```

**Option B: Validate before running**:
```bash
clapi --config clapi.toml --validate-only
```

### 9. Port Already in Use

**Error**:
```
Error: Address already in use (os error 98)
```

**Cause**: Port 8080 already bound.

**Solutions**:

**Option A: Use different port**:
```toml
[server]
listen_addr = "0.0.0.0:8081"
```

**Option B: Kill existing process**:
```bash
# Find process using port 8080
sudo lsof -i :8080

# Kill process
kill -9 <PID>
```

**Option C: Check for zombie processes**:
```bash
ps aux | grep clapi
```

### 10. Out of Memory (OOM)

**Error**:
```
Error: Cannot allocate memory
```

**Cause**: `max_budget_slots` too large for available RAM.

**Solutions**:

**Option A: Reduce slot count**:
```toml
[server]
max_budget_slots = 1_000_000  # 128MB (not 100M = 12.8GB)
```

**Option B: Check actual usage**:
```bash
curl http://localhost:8080/metrics | jq '.budget.slots.utilization'
```

**Option C: Increase system memory**:
```bash
# Check available memory
free -h

# Check swap
swapon -s
```

## Performance Issues

### Slow Request Processing

**Symptoms**: Requests taking >1s when provider responds in <100ms.

**Diagnostics**:
```bash
# Check hot-path latency
curl http://localhost:8080/metrics | jq '.latency'
```

**Expected**:
- Budget check: <100ns
- Provider routing: <100ns
- Metrics tracking: <20ns
- Total overhead: <300ns

**Solutions**:

**Option A: Enable trace logging**:
```bash
RUST_LOG=trace clapi --config clapi.toml 2>&1 | grep "latency"
```

**Option B: Check CPU usage**:
```bash
top -p $(pgrep clapi)
```

**Option C: Run benchmarks**:
```bash
cd clapi_core
cargo bench
```

### High Memory Usage

**Symptoms**: Memory usage >expected (slots × 128B).

**Diagnostics**:
```bash
# Check actual slot count
curl http://localhost:8080/metrics | jq '.budget.slots.active_count'

# Check process memory
ps aux | grep clapi
```

**Solutions**:

**Option A: Clean up inactive budgets**:
```bash
curl -X POST http://localhost:8080/admin/budget/cleanup \
  -H "Authorization: Bearer ${ADMIN_API_KEY}" \
  -d '{"inactive_days": 7}'
```

**Option B: Reduce retention**:
```toml
[metrics]
retention_days = 30  # Reduce from 90 days

[audit]
retention_days = 365  # Reduce from 7 years (if not required)
```

### High CPU Usage

**Symptoms**: CPU usage >20% with light load.

**Diagnostics**:
```bash
# Check request rate
curl http://localhost:8080/metrics | jq '.circuit_breaker.total_requests'

# Profile with perf
perf record -p $(pgrep clapi) -g -- sleep 30
perf report
```

**Solutions**:

**Option A: Disable verbose logging**:
```bash
RUST_LOG=info clapi --config clapi.toml  # Not trace/debug
```

**Option B: Reduce metrics export frequency**:
```toml
[metrics]
export_interval_secs = 300  # 5 minutes (not 60s)
```

**Option C: Disable audit logging** (if not needed):
```toml
[audit]
enabled = false
```

## Migration Issues

### Phase 1 → Phase 2 (HTTP Proxy Added)

**Issue**: Incompatible API changes.

**Solution**: Update client code:

**Before (Phase 1)**:
```rust
let registry = BudgetRegistry::new();
registry.allocate("budget_id", 100_00)?;
```

**After (Phase 2)**:
```rust
// Use HTTP API
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer budget_id"
```

### Phase 2 → Phase 3 (Hash Integrity Added)

**Issue**: Hash chain verification errors on startup.

**Solution**: Rebuild hash chain:
```bash
curl -X POST http://localhost:8080/admin/audit/rebuild \
  -H "Authorization: Bearer ${ADMIN_API_KEY}"
```

### Phase 3 → Phase 4 (Compliance Audit Added)

**Issue**: New audit log format incompatible.

**Solution**: Export old logs before upgrade:
```bash
# Before upgrade
curl http://localhost:8080/audit/export?format=json > audit_v0.3.0.json

# After upgrade
curl -X POST http://localhost:8080/admin/audit/import \
  -H "Authorization: Bearer ${ADMIN_API_KEY}" \
  -d @audit_v0.3.0.json
```

### Phase 4 → Phase 4.5 (Metrics/Forecasting Added)

**Issue**: No breaking changes, metrics enabled by default.

**Solution**: Disable if not needed:
```toml
[metrics]
enabled = false
```

## Debugging Techniques

### Enable Trace Logging

```bash
RUST_LOG=trace clapi --config clapi.toml
```

**Log levels**:
- `error`: Critical errors only
- `warn`: Warnings + errors
- `info`: Informational (default)
- `debug`: Detailed debug info
- `trace`: All operations (verbose)

### Inspect Atomic State

```bash
# Dump all budget slots (admin only)
curl http://localhost:8080/admin/debug/slots \
  -H "Authorization: Bearer ${ADMIN_API_KEY}"
```

### Analyze Metrics

```bash
# Pretty-print all metrics
curl http://localhost:8080/metrics | jq .

# Filter specific category
curl http://localhost:8080/metrics | jq '.circuit_breaker'

# Watch live updates
watch -n 5 'curl -s http://localhost:8080/metrics | jq .circuit_breaker'
```

### Test Circuit Breaker

```bash
# Trigger failures manually
for i in {1..20}; do
  curl -X POST http://localhost:8080/admin/circuit/inject_failure \
    -H "Authorization: Bearer ${ADMIN_API_KEY}" \
    -d '{"provider_id": "anthropic"}'
done

# Verify circuit opened
curl http://localhost:8080/metrics/circuit_breaker
```

### Validate Hash Chain

```bash
# Verify integrity
curl http://localhost:8080/audit/verify

# Export for external validation
curl http://localhost:8080/audit/export?format=json | \
  jq -r '.[] | .hash' | \
  sha256sum --check
```

### Run Property Tests Locally

```bash
# 1000-thread concurrent allocation test
cargo test --test proxy_property_tests -- --nocapture

# Stress test (1M allocations)
cargo test --test proxy_stress_tests -- --ignored --nocapture

# All tests with verbose output
cargo test -- --nocapture
```

### Benchmark Performance

```bash
# Run all benchmarks
cargo bench

# Specific benchmark
cargo bench --bench budget_metacapsule_bench

# Compare with baseline
cargo bench -- --save-baseline phase2
cargo bench -- --baseline phase2
```

## Getting Help

### Check Documentation

1. **Quick Start**: [QUICK_START.md](QUICK_START.md)
2. **Configuration**: [CONFIGURATION.md](CONFIGURATION.md)
3. **Metrics Guide**: [METRICS_ADMIN_GUIDE.md](METRICS_ADMIN_GUIDE.md)
4. **Architecture**: [../README.md](../README.md)

### Search Issues

Check existing issues: https://github.com/primitives/clapi_core/issues

### Report a Bug

Include:
1. Error message (full stack trace)
2. Configuration (redact API keys)
3. Clapi Core version: `clapi --version`
4. Rust version: `rustc --version`
5. OS: `uname -a`
6. Steps to reproduce

### Ask Questions

- **GitHub Discussions**: https://github.com/primitives/clapi_core/discussions
- **Email**: samuel@primitives.io
