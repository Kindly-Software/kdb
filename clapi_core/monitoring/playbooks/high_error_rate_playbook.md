# Incident Playbook: High Error Rate

## Alert: HighErrorRate (Error rate >5%)
**Severity**: CRITICAL

## Initial Response (1 minute)

```bash
# Check current error rate
curl http://localhost:8080/metrics | grep -E "(proxy_errors_total|proxy_requests_total)"

# Calculate: errors / requests * 100
```

## Root Cause Analysis

### Scenario 1: Provider Errors (5xx)
**Symptoms**: Provider returning 5xx errors

**Check**:
```bash
# Check provider-specific error rates
curl http://localhost:8080/metrics | grep provider_requests_failure_total

# Identify which provider(s) failing
```

**Action**: Circuit breaker should auto-open, failover to healthy providers

### Scenario 2: Budget Exhaustion
**Symptoms**: 403 Forbidden (budget exhausted)

**Check**:
```bash
# Check budget exhaustion count
curl http://localhost:8080/metrics | grep budget_exhausted_count

# If high, increase budgets or add refill
```

**Fix**: See [Budget Exhaustion Playbook](budget_exhaustion_playbook.md)

### Scenario 3: OAuth Verification Failures
**Symptoms**: 401 Unauthorized (invalid tokens)

**Check**:
```bash
# Check OAuth failure rate
curl http://localhost:8080/metrics | grep oauth_verification_failures_total
```

**Fix**: See [OAuth Failure Playbook](oauth_failure_playbook.md)

### Scenario 4: Rate Limiting
**Symptoms**: 429 Too Many Requests

**Check**:
```bash
# Check rate limit violations
curl http://localhost:8080/metrics | grep rate_limit_exceeded_total
```

**Fix**: Increase rate limits or distribute load

## Mitigation

### Immediate
1. **Circuit breaker**: Let it fail over to healthy providers
2. **Manual provider switch**: Force route to known-good provider
3. **Increase error threshold**: Temporary relief (not recommended)

### Short-term
1. **Provider health checks**: Disable unhealthy providers
2. **Retry logic**: Enable retries for transient errors
3. **Client communication**: Notify users of degraded service

### Long-term
1. **Error categorization**: Distinguish transient vs permanent
2. **Graceful degradation**: Fallback responses
3. **SLO adjustment**: Review if targets realistic

## Verification
```bash
# Error rate should drop to <0.1%
curl http://localhost:8080/metrics | grep proxy_errors_total

# Monitor over 5 minutes
watch -n 10 'curl -s http://localhost:8080/metrics | grep proxy_errors_total'
```

## Related Playbooks
- [Circuit Breaker Open](circuit_open_playbook.md)
- [Budget Exhaustion](budget_exhaustion_playbook.md)
- [OAuth Failure](oauth_failure_playbook.md)
