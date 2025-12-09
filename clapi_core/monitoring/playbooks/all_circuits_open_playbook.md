# Incident Playbook: All Circuits Open (Emergency)

## Alert: AllCircuitsOpen (All 16 providers circuit open)
**Severity**: CRITICAL (service degraded)

## EMERGENCY RESPONSE (0-2 minutes)

This is the **worst-case scenario**: all providers failing. Service is degraded.

### 1. Immediate Action
```bash
# Verify all circuits open
curl http://localhost:8080/metrics/circuit_breaker | grep "clapi_circuit_state 2" | wc -l

# Expected: 16 (all providers open)
```

### 2. Check Provider Status Pages
- Anthropic: https://status.anthropic.com
- OpenAI: https://status.openai.com
- Google: https://status.cloud.google.com
- Cohere: https://status.cohere.ai

**If global outage**: Wait for provider recovery (nothing we can do)
**If only we're affected**: Network/config issue on our side

### 3. Manual Provider Test
```bash
# Test each provider manually
curl -X POST https://api.anthropic.com/v1/messages \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-3-5-sonnet-20241022","max_tokens":10,"messages":[{"role":"user","content":"test"}]}'

# If this works but circuit is open, clapi_core bug
```

## Root Cause Analysis (2-10 minutes)

### Scenario 1: Global Provider Outage
**Symptoms**: All provider status pages show incidents

**Action**: Wait for provider recovery, monitor status pages

### Scenario 2: Network Outage
**Symptoms**: Cannot reach provider endpoints

**Check**:
```bash
# Test network connectivity
ping api.anthropic.com
ping api.openai.com

# Check DNS
nslookup api.anthropic.com

# Check routing
traceroute api.anthropic.com
```

**Fix**: Diagnose network issue, contact network team

### Scenario 3: API Key Invalidation
**Symptoms**: All providers return 401 Unauthorized

**Check**: API keys revoked or rotated
```bash
# Check auth headers in logs
journalctl -u clapi_core -n 100 | grep -i "401\|unauthorized"
```

**Fix**: Update API keys in config, restart service

### Scenario 4: Circuit Breaker Bug
**Symptoms**: Providers healthy but circuits open

**Check**: Failure rate metrics vs actual provider health
```bash
# Check failure rates
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_failure_rate_bp

# If all showing >10% but providers responding, bug
```

**Fix**: Manually force circuits closed (emergency only)

## Emergency Mitigation

### Option 1: Manual Circuit Override (DANGEROUS)
```bash
# Force all circuits closed (ONLY if providers confirmed healthy)
for i in {0..15}; do
  curl -X POST http://localhost:8080/admin/circuit_breaker/force_close \
    -H "Content-Type: application/json" \
    -d "{\"provider_id\": $i}"
done

# Verify state changed
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_state
```

**WARNING**: Only do this if you're **100% certain** providers are healthy. Forcing circuits closed when providers are failing will cause cascading failures.

### Option 2: Single Provider Bypass
```bash
# Test which provider is healthiest
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_failure_rate_bp

# Force close only the healthiest provider
curl -X POST http://localhost:8080/admin/circuit_breaker/force_close \
  -H "Content-Type: application/json" \
  -d '{"provider_id": 0}'  # Anthropic (example)
```

### Option 3: Emergency Maintenance Mode
```bash
# Put service in maintenance mode (return cached responses)
curl -X POST http://localhost:8080/admin/maintenance_mode/enable

# Notify users
echo "API temporarily in maintenance mode" > /var/www/html/status.html
```

## Communication (CRITICAL)

### Internal (Immediate)
- Post in `#critical-incidents` Slack
- Tag `@oncall-engineer`, `@engineering-lead`, `@cto`
- Page on-call engineer immediately
- Update incident status every 2 minutes

### External (Within 5 minutes)
- Post status page: "API experiencing issues with provider connectivity"
- Email affected customers (high-priority accounts first)
- Social media update (if applicable)

## Recovery Timeline

### Provider Recovery Scenario
1. **T+0**: All circuits open
2. **T+60s**: Circuit cooldown expires, enter HalfOpen
3. **T+60-120s**: Test requests to providers
4. **T+120s**: If providers healthy, circuits close
5. **T+180s**: Full recovery

### Manual Recovery Scenario
1. **T+0**: Force circuits closed (emergency)
2. **T+0**: Immediate traffic routing
3. **T+0-5min**: Monitor for re-trip
4. **T+5min**: If stable, recovery complete

## Verification (After Recovery)

```bash
# All circuits should be closed
curl http://localhost:8080/metrics/circuit_breaker | grep "clapi_circuit_state 0" | wc -l
# Expected: 16

# Failure rates should be <5%
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_failure_rate_bp
# All should be <500 (5%)

# Proxy latency should be normal
curl http://localhost:8080/metrics | grep clapi_proxy_latency_ns
```

## Post-Incident (Within 1 hour)

1. **Incident report**: Write detailed timeline
2. **Root cause**: Why did ALL providers fail?
3. **SLO impact**: How much availability SLO violated?
4. **Customer impact**: How many requests failed?
5. **Preventive measures**: How to prevent total failure?

## Preventive Measures

1. **Multi-region providers**: Geographic diversity
2. **Circuit breaker tuning**: Lower threshold for faster detection
3. **Health monitoring**: Proactive provider health checks
4. **Fallback mechanisms**: Cached responses, degraded mode
5. **Redundant infrastructure**: Multiple clapi_core clusters

## Related Playbooks
- [Circuit Breaker Open](circuit_open_playbook.md) (single provider)
- [Service Down](service_down_playbook.md)
- [High Error Rate](high_error_rate_playbook.md)

## Framework Compliance
- **UCE34 Q30-Q32**: Emergency recovery validated
- **I20 Q16-Q20**: Crisis management validated
- **T28 Q22-Q28**: Chaos engineering validated
