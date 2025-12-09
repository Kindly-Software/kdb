# Incident Playbook: CPU Saturation

## Alert: HighCPUUsage (CPU >90% sustained)
**Severity**: CRITICAL

## Initial Response (1 minute)

```bash
# Check CPU usage
curl http://localhost:8080/metrics | grep clapi_cpu_usage_percent

# Check thread count
curl http://localhost:8080/metrics | grep clapi_threads_active
```

## Root Cause Analysis

### Scenario 1: High Request Load
**Symptoms**: High request rate, CPU correlates with traffic

**Action**:
```bash
# Check request rate
curl http://localhost:8080/metrics | grep proxy_requests_total

# If >10K req/s, scale horizontally
```

**Fix**: Add more clapi_core instances, load balance

### Scenario 2: CAS Contention
**Symptoms**: High retry rate on atomic operations

**Check**:
```bash
# Check CAS retry rate
curl http://localhost:8080/metrics | grep budget_cas_retries_total

# If >10% retry rate, high contention
```

**Fix**: Reduce contention (shard budgets, scale horizontally)

### Scenario 3: Hash Computation
**Symptoms**: CPU spike on hash-heavy operations

**Check**: SIMD hashing enabled (may be slower under load)
```bash
# Check if simd-hashing feature enabled
cargo features --package clapi_core | grep simd-hashing

# If enabled, disable (scalar hashing 15.6× faster under load)
```

**Fix**: Disable `simd-hashing`, use `const-hashing` only

### Scenario 4: Thermal Throttling
**Symptoms**: CPU frequency reduced

**Check**:
```bash
# Check CPU frequency
curl http://localhost:8080/metrics | grep node_cpu_frequency_hertz

# Compare to max frequency
# If <90% of max, thermal throttling
```

**Fix**: Improve cooling, reduce ambient temperature

## Mitigation

### Immediate
1. **Scale horizontally**: Add more instances
   ```bash
   # Add 2 more instances
   docker-compose up --scale clapi_core=3
   ```

2. **Rate limiting**: Reduce incoming traffic
   ```toml
   [rate_limiting]
   max_requests_per_sec = 5000  # Reduce from 10K
   ```

### Long-term
1. **Profiling**: Use `perf` to identify hot paths
   ```bash
   perf record -g -p $(pgrep clapi_core)
   perf report
   ```

2. **Optimize hot paths**: Reduce CPU-intensive operations
3. **Auto-scaling**: Scale on CPU utilization

## Verification
```bash
# CPU should drop to <70%
watch -n 5 'curl -s http://localhost:8080/metrics | grep clapi_cpu_usage_percent'
```

## Related Playbooks
- [Memory Leak](memory_leak_playbook.md)
- [High Contention](high_contention_playbook.md)
- [Latency Spike](latency_spike_playbook.md)
