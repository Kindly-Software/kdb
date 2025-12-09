# Incident Playbook: Circuit Breaker Open

## Alert Triggers
- **AllCircuitsOpen**: All 16 providers circuit open
- **SomeCircuitsOpen**: >5 providers circuit open

## Severity
- **AllCircuitsOpen**: CRITICAL (service degraded)
- **SomeCircuitsOpen**: WARNING

## Initial Response (First 2 minutes)

### 1. Verify Circuit State
```bash
# Check which circuits are open
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_state

# State codes:
# 0 = Closed (healthy)
# 1 = HalfOpen (monitoring recovery)
# 2 = Open (provider failing)
```

### 2. Check Failure Rate
```bash
# Check failure rate for each provider (in basis points, 0-10000)
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_failure_rate_bp

# Thresholds:
# <500 bp (5%): Closed
# 500-1000 bp (5-10%): HalfOpen
# >1000 bp (10%): Open
```

### 3. Check Provider Status
```bash
# Manual check of provider endpoints
curl https://api.anthropic.com/v1/health
curl https://api.openai.com/v1/models
curl https://generativelanguage.googleapis.com/v1/models
```

## Root Cause Analysis (2-10 minutes)

### Scenario 1: Provider Outage
**Symptoms:**
- Provider endpoint returns 5xx errors
- Provider status page shows incident

**Action:**
1. Confirm provider outage on status pages:
   - Anthropic: https://status.anthropic.com
   - OpenAI: https://status.openai.com
   - Google: https://status.cloud.google.com
2. Wait for circuit breaker to auto-recover (60s cooldown)
3. Failover to healthy providers automatically handled

### Scenario 2: Network Issue
**Symptoms:**
- Connection timeout errors
- Provider endpoints unreachable

**Action:**
```bash
# Test network connectivity
ping api.anthropic.com
traceroute api.anthropic.com

# Check DNS resolution
nslookup api.anthropic.com

# Check firewall/proxy logs
journalctl -u clapi_core -n 100 | grep -i "connection refused\|timeout"
```

### Scenario 3: Rate Limiting
**Symptoms:**
- Provider returns 429 (Too Many Requests)
- Failure rate spikes during high load

**Action:**
1. Check request rate:
   ```bash
   curl http://localhost:8080/metrics | grep clapi_provider_requests_total
   ```
2. Reduce request rate (enable rate limiting)
3. Distribute load across more providers

### Scenario 4: Configuration Error
**Symptoms:**
- Circuit opens immediately after deployment
- Invalid API keys or endpoints

**Action:**
```bash
# Check config
cat /etc/clapi/clapi.toml

# Verify API keys valid
# Check endpoint URLs correct
```

## Mitigation Steps

### Immediate (0-2 minutes)
1. **Automatic failover**: Circuit breaker auto-routes to healthy providers
2. **Monitor recovery**: Watch for circuit state transitions (Open → HalfOpen → Closed)

### Manual Override (if all circuits open)
```bash
# Manually force a circuit closed (emergency only)
# WARNING: Only do this if you're SURE provider is healthy
curl -X POST http://localhost:8080/admin/circuit_breaker/force_close \
  -H "Content-Type: application/json" \
  -d '{"provider_id": 0}'

# Verify state changed
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_state
```

### Short-term (2-30 minutes)
1. **Circuit tuning**: Adjust thresholds if too sensitive
   - Increase failure threshold (default 10%)
   - Increase cooldown period (default 60s)
   - Increase min_samples (default 10)

2. **Add provider capacity**: Enable more providers for redundancy

### Long-term (1-7 days)
1. **Multi-region providers**: Add geographic diversity
2. **Synthetic monitoring**: Proactive provider health checks
3. **Auto-scaling**: Scale horizontally on high load

## Circuit Breaker Configuration

Edit `/etc/clapi/clapi.toml`:

```toml
[circuit_breaker]
failure_threshold_bp = 1000  # 10% (increase if too sensitive)
recovery_threshold_bp = 500   # 5%
cooldown_secs = 60            # Increase to prevent flapping
min_samples = 10              # Increase for more stable detection
```

## Recovery Timeline

### Normal Recovery (60s cooldown)
1. **T+0s**: Circuit opens (failure rate >10%)
2. **T+60s**: Circuit enters HalfOpen (monitoring recovery)
3. **T+60-120s**: If failure rate <5%, circuit closes
4. **T+120s**: Full recovery

### Manual Recovery (emergency)
1. **T+0s**: Admin forces circuit closed
2. **T+0s**: Immediate traffic routing
3. **Monitor**: Watch for re-trip if provider still unhealthy

## Communication

### Internal
- Post in `#clapi-alerts` Slack channel
- Tag `@oncall-engineer`
- Update every 5 minutes if AllCircuitsOpen

### External
- If AllCircuitsOpen >5 minutes, post status page update
- Email affected customers if SLO violated

## Verification (After Recovery)

```bash
# Verify all circuits healthy
curl http://localhost:8080/metrics/circuit_breaker | grep "clapi_circuit_state 0"

# Should see 16 providers with state=0 (Closed)

# Check failure rates back to normal (<5%)
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_failure_rate_bp
```

## Rollback Plan

If mitigation causes more issues:
1. **Revert config changes**: Git revert to last known good config
2. **Restart service**: Cold restart to reset circuit state
3. **Manual routing**: Route to specific known-good provider

## Post-Incident Review (Within 24 hours)

1. **Root cause**: Why did circuits open?
2. **Provider impact**: Which providers affected?
3. **Failover success**: Did automatic failover work?
4. **SLO impact**: Did we violate availability SLO?
5. **Preventive measures**: How to prevent future incidents?

## Related Playbooks
- [Latency Spike](latency_spike_playbook.md)
- [High Error Rate](high_error_rate_playbook.md)
- [Service Down](service_down_playbook.md)

## Framework Compliance
- **UCE34 Q10-Q12**: Atomic capsule circuit breaker
- **I20 Q11-Q15**: Safe failover behavior
- **T28 Q22-Q28**: Production circuit testing
