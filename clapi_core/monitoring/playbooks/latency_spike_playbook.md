# Incident Playbook: P50 Latency Spike

## Alert Triggers
- **ProxyLatencyP50Exceeded**: P50 >15ms (target <10ms)
- **ProxyLatencyP99Exceeded**: P99 >200ms (target <100ms)
- **ProxyLatencyP50Trending**: P50 trending up >20%

## Severity
- **P50 >15ms**: CRITICAL
- **P99 >200ms**: CRITICAL
- **P50 trending up >20%**: WARNING

## Initial Response (First 5 minutes)

### 1. Verify Alert
```bash
# Check current P50/P99 latency
curl http://localhost:8080/metrics | grep clapi_proxy_latency

# Grafana: dashboard_proxy_performance.json
# Look at "Proxy Latency (P50, P95, P99, P999)" panel
```

### 2. Check Provider Health
```bash
# Check if provider latency is the cause
curl http://localhost:8080/metrics | grep clapi_proxy_provider_latency

# Expected: Provider latency should be 50-100ms (typical)
# If provider latency >200ms, provider issue (not clapi_core)
```

### 3. Check Circuit Breaker State
```bash
# Check if any circuits are open (failover latency)
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_state

# State values: 0=Closed, 1=HalfOpen, 2=Open
# If multiple circuits open, failover may be causing latency
```

## Root Cause Analysis (5-15 minutes)

### Scenario 1: Provider Latency Spike
**Symptoms:**
- Provider latency >200ms
- clapi_core hot path overhead <300ns (normal)

**Action:**
1. Check provider status pages (Anthropic, OpenAI, Google)
2. Switch to healthier provider manually if needed
3. Wait for provider recovery (circuit breaker will auto-recover)

### Scenario 2: Hot Path Overhead
**Symptoms:**
- Provider latency normal (50-100ms)
- clapi_core overhead >300ns

**Action:**
```bash
# Check hot path breakdown
curl http://localhost:8080/metrics | grep -E "(budget_validation|routing_decision|circuit_breaker_check|audit_log_write)_latency"

# Expected:
# - Budget validation: <60ns
# - Routing decision: <80ns
# - Circuit breaker check: <20ns
# - Audit logging: <50ns (async)
```

**If budget validation >100ns:**
- High CAS contention (check `clapi_budget_cas_retries_total`)
- Scale horizontally (add more instances)

**If routing decision >100ns:**
- Provider health checks slow
- Reduce health check frequency

### Scenario 3: Circuit Flapping
**Symptoms:**
- Frequent circuit state changes (Closed → HalfOpen → Open)
- Failover latency spikes

**Action:**
1. Identify flapping provider:
   ```bash
   curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_trips_total
   ```
2. Manually disable flapping provider in config
3. Increase cooldown period (default 60s)

### Scenario 4: Resource Saturation
**Symptoms:**
- CPU >90%
- Memory growth >100MB/hour

**Action:**
```bash
# Check CPU/memory
curl http://localhost:8080/metrics | grep -E "(cpu_usage_percent|memory_bytes)"

# If CPU >90%, scale horizontally
# If memory leak, restart service (hot restart, zero downtime)
```

## Mitigation Steps

### Immediate (0-5 minutes)
1. **Scale horizontally**: Add more clapi_core instances
2. **Manual provider switch**: Route to healthier provider
3. **Increase timeout**: Temporary relief (not recommended long-term)

### Short-term (5-30 minutes)
1. **Circuit breaker tuning**: Adjust thresholds
2. **Rate limiting**: Reduce load on struggling providers
3. **Caching**: Enable response caching (if applicable)

### Long-term (1-7 days)
1. **Provider diversity**: Add more providers for redundancy
2. **Performance optimization**: Profile hot path, optimize capsules
3. **SLO adjustment**: Review if targets are realistic

## Communication

### Internal
- Post in `#clapi-incidents` Slack channel
- Tag `@oncall-engineer` and `@engineering-lead`
- Update incident status every 15 minutes

### External (if customer-facing)
- Post status page update if latency >100ms sustained
- Notify affected customers via email if SLO violated

## Verification (After Mitigation)

```bash
# Verify P50 back to <10ms
curl http://localhost:8080/metrics | grep clapi_proxy_latency_ns | grep 0.5

# Verify SLO compliance
# Grafana: dashboard_proxy_performance.json → "SLO Compliance (P50 <10ms)"
# Should show >99.5% compliance
```

## Rollback Plan

If mitigation makes it worse:
1. **Revert config changes**: Git revert to last known good config
2. **Restart service**: Cold restart to clear state
3. **Failover to backup cluster**: If available

## Post-Incident Review (Within 24 hours)

1. **Root cause**: What caused the latency spike?
2. **Detection time**: How long until alert fired?
3. **Resolution time**: How long to mitigate?
4. **Preventive measures**: What can prevent this in future?
5. **SLO impact**: Did we violate SLO? How much error budget consumed?

## Related Playbooks
- [Circuit Breaker Open](circuit_open_playbook.md)
- [High Error Rate](high_error_rate_playbook.md)
- [CPU Saturation](cpu_saturation_playbook.md)

## Framework Compliance
- **UCE34 Q30-Q32**: Production monitoring validated
- **I20 Q16-Q20**: Incident response process validated
- **T28 Q28**: Observability metrics validated
