# MCP Server Incident Response Runbook

**Framework**: UCE34 + B32
**Severity Levels**: P0 (Critical), P1 (High), P2 (Medium), P3 (Low)
**Last Updated**: 2025-11-16

## Quick Reference

| Incident | Severity | MTTR | Recovery |
|----------|----------|------|----------|
| Service down | P0 | <5min | Automatic rollback + manual restart |
| Deployment failed | P1 | <10min | Check logs, fix issue, retry |
| Health check fails | P0 | <5min | Rollback, restart service |
| High latency | P2 | <30min | Check logs, restart if needed |
| Memory leak | P1 | <1hour | Restart, investigate root cause |
| Disk full | P1 | <30min | Clean up, extend disk |

## P0: Service Down

**Impact**: MCP server not responding, all users affected
**Target MTTR**: 5 minutes

### Detection
```bash
# Service not active
ssh samuel@192.168.0.38 'sudo systemctl is-active mcp-debug'
# Result: inactive

# Health endpoint not responding
curl -s http://192.168.0.38:5678/health
# Result: Connection refused
```

### Immediate Response (Step 1: Check Logs)
```bash
# Get recent logs (last 20 lines)
ssh samuel@192.168.0.38 'sudo journalctl -u mcp-debug -n 20 --no-pager'

# Look for error patterns:
# - panic: indicates crash
# - OOM: memory exhaustion
# - segfault: memory corruption
# - error: startup failure
```

### Recovery Option 1: Automatic Rollback (Recommended)
```bash
cd /home/samuel/Primitives/atomic_mcp_server

# Rollback to previous version
./deploy.sh rollback

# Confirm when prompted
```

**Expected Output**:
```
[INFO] INITIATING ROLLBACK
[INFO] Backup found
[INFO] Service stopped
[INFO] Binary restored
[INFO] Service started
✅ Health check passed
✅ Rollback successful
```

**Failure Indicators**:
- `Health check failed after 10 attempts` → Go to Option 2
- `Rollback failed` → Go to Manual Restart

### Recovery Option 2: Manual Service Restart
```bash
# SSH to server
ssh samuel@192.168.0.38

# Restart service
sudo systemctl restart mcp-debug
sleep 3

# Check status
sudo systemctl status mcp-debug

# Verify health
curl -s http://localhost:5678/health | jq .
```

**Expected Output**:
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

### Recovery Option 3: Manual Service Start
```bash
ssh samuel@192.168.0.38

# Check if service exists
sudo systemctl list-unit-files | grep mcp-debug

# If exists but inactive:
sudo systemctl start mcp-debug
sudo systemctl is-active mcp-debug

# If fails:
# 1. Check logs (see Step 1 above)
# 2. Check binary: ls -la /usr/local/bin/mcp_debug_server
# 3. Check service file: cat /etc/systemd/system/mcp-debug.service
# 4. Manually run binary: /usr/local/bin/mcp_debug_server
```

### Verification
```bash
# Confirm service is healthy
./deploy.sh health

# Expected output:
# ✅ Health check passed

# Check recent logs
ssh samuel@192.168.0.38 'sudo journalctl -u mcp-debug -n 10 --no-pager'
```

### Post-Incident
- [ ] Determine root cause (see Troubleshooting below)
- [ ] Update logs/notes
- [ ] Schedule post-mortem if needed
- [ ] Implement preventive measures

---

## P1: Deployment Failed

**Impact**: Unable to deploy new code, deployment pipeline blocked
**Target MTTR**: 10 minutes

### Check Deployment Exit Code

```bash
# After running ./deploy.sh, note the exit code:
echo $?

# Exit codes:
# 0 = Success
# 1 = Pre-flight checks failed (git dirty, ssh timeout, etc.)
# 2 = rsync sync failed
# 3 = Service start failed
# 4 = Health check failed (automatic rollback)
# 130 = Interrupted by user
# 99 = CRITICAL: Rollback failed
```

### Exit Code 1: Pre-Flight Checks Failed

**Likely Causes**:
- Git working directory dirty
- SSH connection failed
- Required command not found
- Remote disk full

**Recovery**:
```bash
# Option 1: Git issue
git status
git add . && git commit -m "WIP: fix deployment"
./deploy.sh

# Option 2: SSH issue
ssh samuel@192.168.0.38 'exit 0'  # Test connectivity
ssh-copy-id samuel@192.168.0.38    # Recopy SSH key if needed
./deploy.sh

# Option 3: Disk full (remote)
ssh samuel@192.168.0.38 'df -h'
ssh samuel@192.168.0.38 'sudo du -sh /tmp/*' # Find large files
ssh samuel@192.168.0.38 'sudo rm -rf /tmp/large-file'  # Clean up
./deploy.sh
```

### Exit Code 2: rsync Sync Failed

**Likely Causes**:
- Remote /tmp directory not writable
- Remote disk full
- SSH disconnected during sync

**Recovery**:
```bash
# Option 1: Clean up remote /tmp
ssh samuel@192.168.0.38 'rm -rf /tmp/mcp_debug_server'

# Option 2: Verify remote disk space
ssh samuel@192.168.0.38 'df -h /tmp'

# Option 3: Manual rsync (verbose)
rsync -avv target/release/mcp_debug_server samuel@192.168.0.38:/tmp/

# Option 4: Retry deployment
./deploy.sh
```

### Exit Code 3: Service Start Failed

**Likely Causes**:
- Binary missing/corrupted
- Systemd service file broken
- Port 5678 already in use

**Recovery**:
```bash
# Check what's using port 5678
ssh samuel@192.168.0.38 'sudo lsof -i :5678'

# Option 1: Kill process on port
ssh samuel@192.168.0.38 'sudo kill -9 $(sudo lsof -t -i :5678)'

# Option 2: Check systemd service
ssh samuel@192.168.0.38 'sudo systemctl status mcp-debug'
ssh samuel@192.168.0.38 'sudo systemctl cat mcp-debug'

# Option 3: Check binary exists
ssh samuel@192.168.0.38 'ls -la /usr/local/bin/mcp_debug_server'
ssh samuel@192.168.0.38 'file /usr/local/bin/mcp_debug_server'

# Option 4: Retry deployment
./deploy.sh
```

### Exit Code 4: Health Check Failed (Auto-Rollback)

**Status**: Automatic rollback has already been attempted

**Verification**:
```bash
# Check if rollback succeeded
./deploy.sh health

# If health check passes, rollback succeeded
# If health check fails, continue to P0 procedures
```

**If Rollback Succeeded**:
```bash
# Investigate why deployment failed
ssh samuel@192.168.0.38 'sudo journalctl -u mcp-debug -n 100 --no-pager'

# Look for:
# - startup errors
# - missing dependencies
# - configuration issues
# - port conflicts

# Fix issue in code, rebuild, retry
./deploy.sh
```

**If Rollback Failed**:
- Follow P0 procedures (Service Down)

### Exit Code 99: CRITICAL - Rollback Failed

**Status**: Deployment failed AND rollback failed - CRITICAL
**Target MTTR**: 30 minutes (manual recovery)

**Immediate Response**:
```bash
# SSH to remote (manual intervention required)
ssh samuel@192.168.0.38

# Check what happened
sudo systemctl status mcp-debug
sudo journalctl -u mcp-debug -n 50

# List backups
ls -la /usr/local/bin/mcp_debug_server*

# Find latest timestamped backup
ls -ltr /usr/local/bin/mcp_debug_server.backup.*
```

**Recovery**:
```bash
# Find a working backup
BACKUP=$(ls -tr /usr/local/bin/mcp_debug_server.backup.* | tail -1)
echo "Restoring from: $BACKUP"

# Stop service
sudo systemctl stop mcp-debug

# Restore
sudo cp "$BACKUP" /usr/local/bin/mcp_debug_server
sudo chown root:root /usr/local/bin/mcp_debug_server
sudo chmod 755 /usr/local/bin/mcp_debug_server

# Start service
sudo systemctl start mcp-debug
sleep 3
sudo systemctl status mcp-debug

# Verify health
curl -s http://localhost:5678/health | jq .
```

---

## P0: Health Check Fails

**Impact**: Service running but not responding to requests
**Target MTTR**: 5 minutes

### Detection
```bash
./deploy.sh health
# Result: ❌ Health check failed
```

### Investigation
```bash
# Check service is running
ssh samuel@192.168.0.38 'sudo systemctl is-active mcp-debug'

# Check HTTP endpoint directly
ssh samuel@192.168.0.38 'curl -v http://localhost:5678/health'

# Check logs for startup errors
ssh samuel@192.168.0.38 'sudo journalctl -u mcp-debug -n 50'

# Check if port is listening
ssh samuel@192.168.0.38 'sudo lsof -i :5678'
```

### Troubleshooting

**Service Running But No Port Listening**:
```bash
ssh samuel@192.168.0.38

# Restart with verbose logging
sudo systemctl restart mcp-debug
sleep 2
sudo journalctl -u mcp-debug -f

# Check for startup errors
# Look for: "error", "panic", "failed to bind", etc.
```

**Port Listening But Health Check Fails**:
```bash
ssh samuel@192.168.0.38

# Test health endpoint directly
curl -v http://localhost:5678/health

# Check response status and content
# Expected: 200 OK with {"status":"ok"}

# If 500 error: check service logs
sudo journalctl -u mcp-debug -f

# If timeout: service might be slow, wait 5s more
sleep 5
curl http://localhost:5678/health
```

**Service Not Responding**:
```bash
# Restart service
ssh samuel@192.168.0.38 'sudo systemctl restart mcp-debug'
sleep 5

# Verify
./deploy.sh health

# If still fails, check for port conflicts
ssh samuel@192.168.0.38 'sudo lsof -i :5678'

# Kill any process on port 5678
ssh samuel@192.168.0.38 'sudo kill -9 $(sudo lsof -t -i :5678)'

# Restart
ssh samuel@192.168.0.38 'sudo systemctl restart mcp-debug'
./deploy.sh health
```

---

## P2: High Latency

**Impact**: MCP requests slow (>1s response time)
**Target MTTR**: 30 minutes

### Detection
```bash
# Measure request latency
ssh samuel@192.168.0.38 'time curl -s http://localhost:5678/health | jq .'

# Expected: <100ms
# Threshold: >1000ms = alarm
```

### Investigation
```bash
# Check system resources
ssh samuel@192.168.0.38 'top -bn1 | head -20'
# Look for: high CPU%, high memory%

# Check disk I/O
ssh samuel@192.168.0.38 'iostat -x 1 3'

# Check network
ssh samuel@192.168.0.38 'netstat -an | grep 5678'

# Check service logs
ssh samuel@192.168.0.38 'sudo journalctl -u mcp-debug -f'
```

### Root Causes & Fixes

**High CPU Usage**:
```bash
# Restart service (may clear deadlock)
ssh samuel@192.168.0.38 'sudo systemctl restart mcp-debug'

# If persists, check for infinite loops in logs
ssh samuel@192.168.0.38 'sudo journalctl -u mcp-debug --since "5 min ago"'
```

**High Memory Usage**:
```bash
# Check memory consumption
ssh samuel@192.168.0.38 'ps aux | grep mcp_debug_server'

# Restart service
ssh samuel@192.168.0.38 'sudo systemctl restart mcp-debug'

# Monitor memory
watch -n 1 'ssh samuel@192.168.0.38 "ps aux | grep mcp"'
```

**Disk I/O Wait**:
```bash
# Check disk usage
ssh samuel@192.168.0.38 'df -h'

# If disk full, clean up
ssh samuel@192.168.0.38 'sudo du -sh /var/log/*'
ssh samuel@192.168.0.38 'sudo journalctl --vacuum=1G'

# Restart service
ssh samuel@192.168.0.38 'sudo systemctl restart mcp-debug'
```

---

## P3: Resource Warnings

**Impact**: Potential future issues, no current impact

### Memory Leak Suspected
```bash
# Monitor memory over time
watch -n 5 'ssh samuel@192.168.0.38 "ps aux | grep mcp_debug_server | grep -v grep"'

# Collect 1-hour baseline
# If memory grows continuously: likely leak
# If memory stable: normal

# Temporary fix: restart periodically
ssh samuel@192.168.0.38 'sudo systemctl restart mcp-debug'

# Permanent fix: investigate and rebuild
# Schedule for next deployment
```

### Disk Space Warnings
```bash
ssh samuel@192.168.0.38 'df -h'

# If >80% usage:
ssh samuel@192.168.0.38 'sudo du -sh /var/log/*'
ssh samuel@192.168.0.38 'sudo journalctl --vacuum=1G'  # Keep 1GB logs
ssh samuel@192.168.0.38 'sudo journalctl -u mcp-debug --vacuum=100M'  # Keep 100MB mcp logs
```

---

## Common Issues & Solutions

### Issue: "SSH connection refused"

**Cause**: SSH key not available or incorrect user

**Solution**:
```bash
# Verify SSH key
ls -la ~/.ssh/id_rsa

# Copy key to remote
ssh-copy-id -i ~/.ssh/id_rsa.pub samuel@192.168.0.38

# Test
ssh samuel@192.168.0.38 'echo OK'
```

### Issue: "Permission denied: /usr/local/bin/mcp_debug_server"

**Cause**: Binary not owned by root or not executable

**Solution**:
```bash
ssh samuel@192.168.0.38
sudo ls -la /usr/local/bin/mcp_debug_server
sudo chown root:root /usr/local/bin/mcp_debug_server
sudo chmod 755 /usr/local/bin/mcp_debug_server
sudo systemctl restart mcp-debug
```

### Issue: "Port 5678 already in use"

**Cause**: Another process bound to port or lingering connection

**Solution**:
```bash
ssh samuel@192.168.0.38

# Find process
sudo lsof -i :5678

# Kill if necessary
sudo kill -9 <PID>

# Wait and restart
sleep 2
sudo systemctl restart mcp-debug
```

### Issue: "systemd service not found"

**Cause**: Service file not installed or wrong name

**Solution**:
```bash
ssh samuel@192.168.0.38

# Check service file
sudo ls -la /etc/systemd/system/mcp-debug.service

# If missing, create it (see DEPLOYMENT.md for details)
# Then:
sudo systemctl daemon-reload
sudo systemctl enable mcp-debug
sudo systemctl start mcp-debug
```

---

## Escalation Procedures

### Cannot Resolve After 30 Minutes
1. Create incident report with:
   - Timestamp
   - Exit code / error message
   - Logs
   - Recovery attempts
2. Escalate to engineering team
3. Schedule post-mortem

### Production Data Impacted
1. Immediately page on-call engineer
2. Gather all logs and backups
3. Do NOT make changes without approval
4. Document all actions

---

## Communication Template

### Incident Started
```
INCIDENT: MCP Server P[0|1|2]
- Service: atomic_mcp_server
- Severity: P[0|1|2]
- Impact: [describe user impact]
- Detected: [timestamp]
- ETA: [time to resolution]
```

### Update During Recovery
```
- Investigating: [what]
- Found: [discovery]
- Taking action: [step]
- ETA: [updated time]
```

### Incident Resolved
```
- Resolved: [timestamp]
- Duration: [how long]
- Root cause: [what failed]
- Fix: [what we did]
- Prevention: [what to change]
```

---

## Pre-Incident Checklist

Before deploying to production, verify:

- [ ] All pre-flight checks pass: `./deploy.sh --dry-run`
- [ ] Code builds cleanly: `cargo build --release`
- [ ] Tests pass locally: `cargo test`
- [ ] Git is clean: `git status` (no uncommitted changes)
- [ ] SSH key works: `ssh samuel@192.168.0.38 'echo OK'`
- [ ] Backup exists on remote: `ssh samuel@192.168.0.38 'ls /usr/local/bin/mcp_debug_server*'`
- [ ] Service is healthy: `./deploy.sh health`

---

## Reference

- **DEPLOYMENT.md** - Full deployment procedures
- **deploy.sh** - Deployment script documentation
- **validate-mcp** - Health check tool
- **/var/log/mcp-deploy.log** - Remote audit log
- **/tmp/mcp-deploy-*.log** - Local deployment logs
