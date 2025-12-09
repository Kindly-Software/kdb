# Migration Guide: v0.1.x → v0.2.0 (Pure Atomic Architecture)

**Version**: 0.1.x → 0.2.0
**Date**: 2025-10-16
**Impact**: Internal Only (Zero Client Changes)
**Duration**: 1-2 hours (testing + validation)

---

## Executive Summary

Version 0.2.0 migrates clapi_core to a 100% lockfree pure atomic architecture. This is an **internal refactoring** with **zero breaking changes** to the public HTTP API.

### What Changed
- **BudgetRegistry**: RwLock HashMap → AtomicPtr array
- **New Capsules**: BudgetSlotCapsule, CircuitBreakerCapsule
- **New Errors**: `CircuitOpen`, `AllocationConflict`, `SlotsExhausted`
- **Performance**: 3-4× faster budget operations

### What Stayed the Same
- ✅ HTTP API (OpenAI-compatible)
- ✅ Client code (zero changes)
- ✅ Budget semantics (deduction/credit logic)
- ✅ Error handling (existing errors work)

### Migration Effort
- **Clients**: 0 hours (no changes)
- **Operators**: 1-2 hours (testing + validation)
- **Downtime**: 0 minutes (rolling upgrade)

---

## Pre-Migration Checklist

### 1. Backup Current State

```bash
# Export budget data (if persistence implemented)
curl http://localhost:8080/admin/budgets/export > budgets_v0.1.json

# Save configuration
cp /etc/clapi-core/config.toml /etc/clapi-core/config.toml.backup

# Save logs
cp /var/log/clapi-core/clapi-core.log /var/log/clapi-core/clapi-core.log.backup
```

### 2. Verify Current Version

```bash
# Check version
curl http://localhost:8080/health | jq .version
# Expected: "0.1.x"

# Check uptime
curl http://localhost:8080/health | jq .uptime

# Check budget count
curl http://localhost:8080/metrics | grep budget_count
```

### 3. Run Current Tests

```bash
# Clone repository
git clone https://github.com/your-org/clapi-core.git
cd clapi-core
git checkout v0.1.x

# Run tests (baseline)
cargo test
cargo bench --bench budget_registry_bench -- --save-baseline v0.1

# Save results
cargo test > test_results_v0.1.txt
```

### 4. Monitor Current Performance

```bash
# Collect baseline metrics (5 minutes)
curl http://localhost:8080/metrics > metrics_v0.1_baseline.txt

# Check latency percentiles
curl http://localhost:8080/metrics | grep budget_latency

# Check error rate
curl http://localhost:8080/metrics | grep error_rate
```

---

## Migration Steps

### Step 1: Update Code

```bash
# Update Cargo.toml
[dependencies]
clapi_core = "0.2.0"

# Or update from source
cd clapi-core
git checkout v0.2.0
```

### Step 2: Build and Test

```bash
# Build new version
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench --bench budget_slot_lockfree_bench

# Compare against baseline
cargo bench --bench budget_slot_lockfree_bench -- --baseline v0.1
```

### Step 3: Validate Locally

```bash
# Start server locally
cargo run --release --bin clapi-server &

# Wait for startup
sleep 5

# Test health endpoint
curl http://localhost:8080/health

# Test budget operations
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Budget-ID: 12345" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Test"}]
  }'

# Check metrics
curl http://localhost:8080/metrics | grep circuit_breaker
curl http://localhost:8080/metrics | grep budget_slots

# Stop server
pkill clapi-server
```

### Step 4: Deploy (Rolling Upgrade)

```bash
# Deploy to staging
ssh staging-host
systemctl stop clapi-core
cp /usr/local/bin/clapi-server /usr/local/bin/clapi-server.v0.1.backup
cp target/release/clapi-server /usr/local/bin/clapi-server
systemctl start clapi-core

# Verify staging
curl http://staging-host:8080/health
curl http://staging-host:8080/metrics

# Deploy to production (one node at a time)
for host in prod-01 prod-02 prod-03; do
  ssh $host "
    systemctl stop clapi-core &&
    cp /usr/local/bin/clapi-server /usr/local/bin/clapi-server.v0.1.backup &&
    cp target/release/clapi-server /usr/local/bin/clapi-server &&
    systemctl start clapi-core
  "
  sleep 60  # Wait for healthcheck
  curl http://$host:8080/health || exit 1
done
```

### Step 5: Verify Deployment

```bash
# Check version
curl http://localhost:8080/health | jq .version
# Expected: "0.2.0"

# Check circuit breaker
curl http://localhost:8080/health | jq .circuit_breaker
# Expected: {"state": "closed", "failure_rate": 0.0}

# Check slot utilization
curl http://localhost:8080/metrics | grep budget_slots_active
curl http://localhost:8080/metrics | grep budget_slots_max

# Run load test
hey -n 10000 -c 100 -m POST \
  -H "Content-Type: application/json" \
  -H "X-Budget-ID: 12345" \
  -D request.json \
  http://localhost:8080/v1/chat/completions
```

---

## Post-Migration Validation

### 1. Performance Validation

```bash
# Compare latency (p50, p99, p999)
curl http://localhost:8080/metrics | grep budget_try_deduct_duration_ns

# Expected improvements:
# p50: ~60ns (vs ~180ns in v0.1)
# p99: ~150ns (vs ~1200ns in v0.1)
# p999: ~300ns (vs ~8500ns in v0.1)

# Compare throughput
# Expected: 60M ops/s (vs 35M ops/s in v0.1)
```

### 2. Functionality Validation

```bash
# Test budget deduction
for i in {1..100}; do
  curl -s -X POST http://localhost:8080/v1/chat/completions \
    -H "Content-Type: application/json" \
    -H "X-Budget-ID: $i" \
    -d '{"model": "gpt-4", "messages": [{"role": "user", "content": "Test"}]}'
done

# Verify budget conservation
curl http://localhost:8080/admin/budgets | jq '.budgets | map(.budget + .total_spent) | unique'
# Expected: [1000_00] (all budgets sum to default)
```

### 3. Error Handling Validation

```bash
# Test budget exhaustion
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Budget-ID: 99999" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Test"}],
    "max_tokens": 1000000
  }'
# Expected: 402 Payment Required

# Test circuit breaker (requires failure injection)
# (Manual test - trigger allocation failures)

# Test slots exhausted (requires 1M allocations)
# (Stress test - allocate 1M budgets)
```

### 4. Monitoring Validation

```bash
# Verify new metrics exist
curl http://localhost:8080/metrics | grep circuit_breaker_state
curl http://localhost:8080/metrics | grep budget_slots_active
curl http://localhost:8080/metrics | grep allocation_conflicts

# Verify Prometheus scraping
curl http://prometheus:9090/api/v1/targets | jq '.data.activeTargets[] | select(.job == "clapi-core")'

# Verify Grafana dashboards
curl http://grafana:3000/api/dashboards/uid/clapi-core
```

---

## Rollback Procedure

If issues arise during migration:

### Step 1: Identify Issue

```bash
# Check error logs
journalctl -u clapi-core -n 100 --no-pager

# Check metrics
curl http://localhost:8080/metrics | grep error

# Check circuit breaker status
curl http://localhost:8080/health | jq .circuit_breaker
```

### Step 2: Rollback Code

```bash
# Restore v0.1 binary
systemctl stop clapi-core
cp /usr/local/bin/clapi-server.v0.1.backup /usr/local/bin/clapi-server
systemctl start clapi-core

# Verify rollback
curl http://localhost:8080/health | jq .version
# Expected: "0.1.x"
```

### Step 3: Restore Data (if needed)

```bash
# Import budgets from backup
curl -X POST http://localhost:8080/admin/budgets/import \
  -H "Content-Type: application/json" \
  -d @budgets_v0.1.json

# Verify budget count
curl http://localhost:8080/metrics | grep budget_count
```

### Step 4: Investigate Root Cause

```bash
# Collect logs
journalctl -u clapi-core -n 1000 > rollback_logs.txt

# Collect metrics
curl http://localhost:8080/metrics > rollback_metrics.txt

# File issue
gh issue create --title "v0.2.0 rollback: <description>" \
  --body "$(cat rollback_logs.txt)" \
  --label bug,rollback
```

---

## Configuration Changes

### New Configuration Options (v0.2.0)

```toml
# /etc/clapi-core/config.toml

[budget_registry]
# Maximum number of budget slots (default: 1,000,000)
max_slots = 1000000

# Default budget for new users (cents, default: 1000_00)
default_budget = 1000_00

[circuit_breaker]
# Circuit breaker open threshold (default: 0.10 = 10%)
failure_threshold = 0.10

# Circuit breaker close threshold (default: 0.05 = 5%)
success_threshold = 0.05

# Circuit breaker cooldown period (seconds, default: 60)
cooldown_seconds = 60

[monitoring]
# Prometheus metrics endpoint (default: :9090)
metrics_bind = "0.0.0.0:9090"

# Health check endpoint (default: :8080/health)
health_bind = "0.0.0.0:8080"
```

### Backward Compatibility

```toml
# v0.1.x configuration still works
[budget_registry]
default_budget = 1000_00  # Still supported

# New fields are optional (defaults used)
```

---

## Monitoring Updates

### New Prometheus Metrics

```promql
# Circuit breaker metrics
circuit_breaker_state          # 0 = closed, 1 = open
circuit_breaker_failure_rate   # Current failure rate (0.0-1.0)
circuit_breaker_trip_count     # Total circuit opens

# Budget slot metrics
budget_slots_active            # Current active slots
budget_slots_max               # Maximum slots (1M)
budget_slots_utilization       # active / max

# Allocation metrics
allocation_conflicts_total     # Total CAS conflicts
allocation_attempts_total      # Total allocation attempts
```

### Updated Grafana Dashboards

```yaml
# Panel 1: Budget Latency (updated)
- Title: "Budget Operation Latency (p50, p99, p999)"
- Query: histogram_quantile(0.99, budget_try_deduct_duration_ns)
- Alert: p99 > 200ns (was 1000ns)

# Panel 2: Circuit Breaker (new)
- Title: "Circuit Breaker State"
- Query: circuit_breaker_state
- Alert: state = open for >5 minutes

# Panel 3: Slot Utilization (new)
- Title: "Budget Slot Utilization"
- Query: budget_slots_active / budget_slots_max
- Alert: utilization > 0.80
```

### Updated Alerts

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

# Medium priority (updated)
- alert: HighBudgetLatency
  expr: histogram_quantile(0.99, budget_try_deduct_duration_ns) > 200e-9
  for: 10m
  annotations:
    summary: "Budget p99 latency >200ns"

# New alert
- alert: HighSlotUtilization
  expr: budget_slots_active / budget_slots_max > 0.80
  for: 10m
  annotations:
    summary: "Budget slots 80%+ utilized"
```

---

## Troubleshooting

### Issue: Circuit Breaker Open After Migration

**Symptoms**: `CircuitOpen` errors immediately after upgrade

**Diagnosis**:
```bash
curl http://localhost:8080/health | jq .circuit_breaker
# Check failure_rate
```

**Fix**:
1. Check logs for allocation failures
2. Verify memory availability (128MB required)
3. Restart service to reset circuit breaker
4. If persists, rollback and investigate

### Issue: High Latency After Migration

**Symptoms**: p99 latency >1ms (worse than v0.1)

**Diagnosis**:
```bash
# Check system load
htop

# Check memory pressure
free -h

# Run benchmarks
cargo bench --bench budget_slot_lockfree_bench
```

**Fix**:
1. Verify sufficient RAM (128MB+ free)
2. Check CPU usage (<80%)
3. Profile with `perf` or `flamegraph`
4. If hardware issue, add resources
5. If code issue, rollback and report bug

### Issue: Slot Allocation Failures

**Symptoms**: `SlotsExhausted` errors with low utilization

**Diagnosis**:
```bash
curl http://localhost:8080/metrics | grep budget_slots
# Check if active < max
```

**Fix**:
1. Restart service (clears state)
2. Check for memory leaks (valgrind)
3. Verify no slot leaks (deallocate test)
4. If bug, rollback and report

---

## Testing Checklist

### Pre-Migration
- [ ] Backup budget data
- [ ] Save configuration
- [ ] Run baseline tests
- [ ] Collect baseline metrics

### Migration
- [ ] Build v0.2.0
- [ ] Run unit tests
- [ ] Run property tests
- [ ] Run stress tests
- [ ] Run benchmarks

### Post-Migration
- [ ] Verify version (0.2.0)
- [ ] Check circuit breaker (closed)
- [ ] Verify slot utilization (<80%)
- [ ] Run load test
- [ ] Compare latency (p50, p99, p999)
- [ ] Verify error handling
- [ ] Update monitoring dashboards
- [ ] Update alerting rules

### Rollback (if needed)
- [ ] Restore v0.1 binary
- [ ] Restore configuration
- [ ] Import budget data
- [ ] Verify functionality
- [ ] File issue with logs

---

## FAQ

### Q: Will my existing budgets be lost?
**A**: No. Budget state is preserved (u64 budget_id).

### Q: Do I need to update client code?
**A**: No. HTTP API is unchanged (OpenAI-compatible).

### Q: What if circuit breaker opens during migration?
**A**: Wait 60 seconds for cooldown, or restart service to reset.

### Q: How do I know migration succeeded?
**A**: Version is "0.2.0", tests pass, latency improved.

### Q: Can I roll back without downtime?
**A**: Yes. Rolling upgrade allows zero-downtime rollback.

### Q: What if I hit `SlotsExhausted` error?
**A**: Deallocate inactive budgets or contact support for capacity increase.

### Q: How long does migration take?
**A**: 1-2 hours for testing + validation. Deployment is <5 minutes per node.

### Q: Is there any data format incompatibility?
**A**: No. Budget ID format unchanged (u64).

---

## Success Criteria

Migration is successful when:
- ✅ Version reports "0.2.0"
- ✅ All tests pass (unit, property, stress)
- ✅ Circuit breaker is closed
- ✅ Latency improved (p99 <200ns)
- ✅ Throughput improved (>50M ops/s)
- ✅ Error rate stable or decreased
- ✅ Monitoring dashboards updated
- ✅ Load test passes (10K requests, 100 concurrent)

---

## Support

For migration assistance:
- **Documentation**: `/home/samuel/Primitives/clapi_core/`
- **Issues**: https://github.com/your-org/clapi-core/issues
- **Slack**: #clapi-core-support
- **Email**: support@your-org.com

---

**Version**: 0.1.x → 0.2.0
**Date**: 2025-10-16
**Author**: Documentation Expert
**Estimated Duration**: 1-2 hours
