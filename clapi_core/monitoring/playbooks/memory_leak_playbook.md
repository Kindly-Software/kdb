# Incident Playbook: Memory Leak

## Alert: MemoryLeak (Memory growth >100MB/hour)
**Severity**: CRITICAL

## Initial Response (2 minutes)

```bash
# Check current memory usage
curl http://localhost:8080/metrics | grep clapi_memory_bytes

# Check growth rate
curl http://localhost:8080/metrics | grep clapi_memory_bytes | awk '{print $2 / 1024 / 1024 " MB"}'
```

## Root Cause Analysis

### Scenario 1: Budget Slot Leak
**Check**: Slots not being deallocated
```bash
# Check active slot count
curl http://localhost:8080/metrics | grep clapi_budget_active_count

# Expected: Should match actual active budgets
# If growing unbounded, deallocation bug
```

**Fix**: Restart service (hot restart, zero downtime)

### Scenario 2: Audit Log Buffer
**Check**: Audit logs accumulating in memory
```bash
# Check audit log size
ls -lh /var/log/clapi_core/audit.log

# If >1GB, rotation not working
```

**Fix**: Enable log rotation, clear old logs

### Scenario 3: Connection Pool Leak
**Check**: HTTP connections not closed
```bash
# Check open file descriptors
lsof -p $(pgrep clapi_core) | wc -l

# Expected: <1000
# If >5000, connection leak
```

**Fix**: Restart HTTP client pool

## Mitigation

### Immediate
1. **Hot restart**: Zero downtime restart
   ```bash
   systemctl reload clapi_core
   ```

2. **Memory limit**: Set cgroup memory limit
   ```bash
   systemctl set-property clapi_core.service MemoryMax=2G
   ```

### Long-term
1. **Profiling**: Use `valgrind` or `heaptrack`
2. **Code review**: Audit deallocation paths
3. **Testing**: Add memory leak tests (T28 Q28)

## Verification
```bash
# Memory should stabilize
watch -n 10 'curl -s http://localhost:8080/metrics | grep clapi_memory_bytes'
```

## Related Playbooks
- [CPU Saturation](cpu_saturation_playbook.md)
- [Service Down](service_down_playbook.md)
