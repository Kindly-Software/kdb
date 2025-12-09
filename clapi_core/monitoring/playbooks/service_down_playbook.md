# Incident Playbook: Service Down

## Alert: ServiceDown (No metrics for >1 minute)
**Severity**: CRITICAL

## Initial Response (30 seconds)

```bash
# Check if service running
systemctl status clapi_core

# Check process
ps aux | grep clapi_core

# Check port binding
netstat -tuln | grep 8080
```

## Root Cause Analysis

### Scenario 1: Process Crashed
**Symptoms**: Service not running

**Check**:
```bash
# Check crash logs
journalctl -u clapi_core -n 100 --since "5 minutes ago"

# Look for panic, segfault, OOM killer
```

**Fix**: Restart service
```bash
systemctl restart clapi_core
```

### Scenario 2: Port Already Bound
**Symptoms**: Service fails to start

**Check**:
```bash
# Check what's using port 8080
lsof -i :8080

# If another process, kill it or change clapi_core port
```

**Fix**: Kill conflicting process or change config

### Scenario 3: Out of Memory
**Symptoms**: OOM killer terminated service

**Check**:
```bash
# Check dmesg for OOM killer
dmesg | grep -i "out of memory\|oom"

# Check available memory
free -h
```

**Fix**: Increase memory limit, restart service

### Scenario 4: Configuration Error
**Symptoms**: Service fails validation on startup

**Check**:
```bash
# Test config validity
clapi_core --config /etc/clapi/clapi.toml --validate

# Check for syntax errors, missing keys
```

**Fix**: Revert to last known good config

## Mitigation

### Immediate (0-1 minute)
1. **Cold restart**:
   ```bash
   systemctl stop clapi_core
   systemctl start clapi_core
   ```

2. **Verify startup**:
   ```bash
   # Wait 10 seconds
   curl http://localhost:8080/health
   ```

### Failover (if restart fails)
1. **Route to backup instance**: Update load balancer
2. **Deploy from backup**: Docker image or binary
3. **Emergency mode**: Minimal config, bypass failing components

### Long-term
1. **Health checks**: Implement liveness/readiness probes
2. **Auto-restart**: Systemd `Restart=always`
3. **Monitoring**: Alert on process death, not just metrics

## Verification
```bash
# Service should be up
systemctl is-active clapi_core

# Metrics should be flowing
curl http://localhost:8080/metrics | head -n 20

# Health endpoint responding
curl http://localhost:8080/health
```

## Rollback Plan
If restart causes more issues:
1. **Revert deployment**: Git revert to last known good
2. **Deploy backup binary**: From artifact storage
3. **Emergency config**: Minimal safe config

## Post-Incident Review
1. **Root cause**: Why did service crash?
2. **Detection time**: How long until alert?
3. **Recovery time**: How long to restore?
4. **Preventive measures**: How to prevent future crashes?

## Related Playbooks
- [Memory Leak](memory_leak_playbook.md)
- [CPU Saturation](cpu_saturation_playbook.md)
- [All Circuits Open](circuit_open_playbook.md)
