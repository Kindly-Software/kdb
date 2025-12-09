# Incident Playbook: High Contention

## Alert: HighContention (CAS retry rate >10%)
**Severity**: WARNING

## Initial Response (1 minute)

```bash
# Check CAS retry rate
curl http://localhost:8080/metrics | grep -E "(budget_cas_retries_total|budget_deductions_total)"

# Calculate: retries / deductions * 100
```

## Root Cause Analysis

### Scenario 1: High Concurrency
**Symptoms**: Many threads competing for same slots

**Check**:
```bash
# Check active thread count
curl http://localhost:8080/metrics | grep threads_active

# Check request rate
curl http://localhost:8080/metrics | grep proxy_requests_total

# If >10K req/s, high concurrency expected
```

**Fix**: Scale horizontally (add more instances)

### Scenario 2: Hot Slot Collision
**Symptoms**: Few budgets receiving most traffic

**Check**: Uneven budget distribution (check if single budget_id dominates)
```bash
# Check budget ID distribution (if logged)
journalctl -u clapi_core -n 1000 | grep budget_id | sort | uniq -c | sort -rn | head -n 10
```

**Fix**: Shard hot budgets across multiple slots

### Scenario 3: Retry Policy Too Aggressive
**Symptoms**: Excessive retry attempts

**Check**: Current retry policy (IMMEDIATE, LIGHT, STANDARD, PERSISTENT)
```bash
# Check retry policy in code
grep "RetryPolicy" /etc/clapi/src/budget/mod.rs
```

**Fix**: Use lighter retry policy (IMMEDIATE → LIGHT)

## Mitigation

### Immediate
1. **Scale horizontally**: Add more clapi_core instances
   ```bash
   docker-compose up --scale clapi_core=3
   ```

2. **Reduce retry attempts**: Lower max retries (default 3)
   ```rust
   // In code
   const MAX_RETRIES: u32 = 2;  // Reduce from 3
   ```

### Short-term
1. **Slot sharding**: Distribute hot budgets across multiple slots
2. **Request batching**: Batch deductions to reduce CAS operations
3. **Caching**: Cache budget state (read-heavy optimization)

### Long-term
1. **Lockfree optimization**: Improve CAS algorithm (reduce contention)
2. **Wait-free structures**: Use wait-free alternatives if possible
3. **Profiling**: Measure contention hotspots with `perf`

## Verification
```bash
# CAS retry rate should drop to <5%
curl http://localhost:8080/metrics | grep budget_cas_retries_total

# Monitor over 10 minutes
watch -n 30 'curl -s http://localhost:8080/metrics | grep budget_cas_retries_total'
```

## Performance Impact

High contention (>10% retry rate):
- **Latency**: +10-50ns per retry
- **Throughput**: -5-15% under load
- **CPU**: +10-20% due to retries

## Related Playbooks
- [CPU Saturation](cpu_saturation_playbook.md)
- [Latency Spike](latency_spike_playbook.md)
- [Budget Exhaustion](budget_exhaustion_playbook.md)
