# Incident Playbook: OAuth Failure

## Alert: OAuthFailureRate (>10% OAuth failures)
**Severity**: CRITICAL

## Initial Response (1 minute)

```bash
# Check OAuth failure rate
curl http://localhost:8080/metrics | grep -E "(oauth_verification_failures_total|oauth_sessions_total)"

# Check KindlyDB connection
curl http://localhost:8080/health | grep kindlydb_status
```

## Root Cause Analysis

### Scenario 1: KindlyDB Connection Lost
**Symptoms**: All OAuth verifications failing

**Action**:
```bash
# Test KindlyDB connectivity
psql -h localhost -U clapi_core -d kindlydb -c "SELECT 1"

# If connection fails, restart KindlyDB or check network
```

**Fix**: Restart KindlyDB connection pool

### Scenario 2: Token Expiry
**Symptoms**: Gradual increase in failures

**Check**:
```bash
# Check token TTL distribution
curl http://localhost:8080/metrics | grep oauth_token_ttl_remaining_ns

# If many tokens near expiry, refresh not working
```

**Fix**: Enable automatic token refresh

### Scenario 3: Invalid Tokens
**Symptoms**: Spike in verification failures

**Check**: Recent authentication changes, token format updates

**Fix**: Invalidate old tokens, force re-authentication

## Mitigation

### Immediate
1. **Restart connection pool**:
   ```bash
   systemctl reload clapi_core
   ```

2. **Clear stale sessions**:
   ```bash
   curl -X POST http://localhost:8080/admin/oauth/cleanup_expired
   ```

### Long-term
1. **Connection pooling**: Persistent KindlyDB connections
2. **Token caching**: Reduce DB queries
3. **Monitoring**: Alert on DB connection issues

## Verification
```bash
# OAuth failure rate should drop to <1%
curl http://localhost:8080/metrics | grep oauth_verification_failures_total
```

## Related Playbooks
- [Payment Failure](payment_failure_playbook.md)
- [Service Down](service_down_playbook.md)
