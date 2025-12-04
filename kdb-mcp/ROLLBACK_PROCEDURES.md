# atomic_mcp_server Rollback Procedures

**Purpose**: Emergency rollback procedures for atomic_mcp_server deployment failures

**Server**: 6900hx-brain (192.168.0.38)

---

## 1. Rollback Decision Tree

```
Deployment Issue?
│
├─> Service won't start → Try Quick Fix (Section 2)
│   │
│   ├─> Fixed? → Continue monitoring
│   └─> Not fixed? → Full Rollback (Section 3)
│
├─> Service crashes repeatedly → Full Rollback (Section 3)
│
├─> High resource usage → Adjust Limits (Section 4) or Full Rollback
│
├─> Security issues → Immediate Rollback (Section 3)
│
└─> Performance issues → Monitor, then decide:
    ├─> Acceptable → Continue with monitoring
    └─> Unacceptable → Full Rollback (Section 3)
```

---

## 2. Quick Fix Procedures

### 2.1 Service Won't Start

**Symptom**: `systemctl status mcp-debug.service` shows `failed` or `inactive`

**Diagnosis**:
```bash
ssh samuel@192.168.0.38 "sudo journalctl -u mcp-debug.service -xe | tail -50"
```

**Common Quick Fixes**:

**Issue: Port Already in Use**
```bash
# Check what's using port 5678
ssh samuel@192.168.0.38 "sudo lsof -i :5678"

# Option 1: Kill conflicting process
ssh samuel@192.168.0.38 "sudo kill <PID>"

# Option 2: Change port in environment file
ssh samuel@192.168.0.38 "sudo sed -i 's/MCP_HTTP_PORT=5678/MCP_HTTP_PORT=5679/' /etc/mcp-debug/mcp-debug.env"

# Restart service
ssh samuel@192.168.0.38 "sudo systemctl restart mcp-debug.service"
```

**Issue: Binary Missing Capabilities**
```bash
# Re-apply capabilities
ssh samuel@192.168.0.38 "sudo setcap cap_sys_ptrace=ep /usr/local/bin/mcp_debug_server"

# Restart service
ssh samuel@192.168.0.38 "sudo systemctl restart mcp-debug.service"
```

**Issue: Permission Denied**
```bash
# Check permissions
ssh samuel@192.168.0.38 "ls -la /usr/local/bin/mcp_debug_server"

# Fix permissions
ssh samuel@192.168.0.38 "sudo chown root:root /usr/local/bin/mcp_debug_server"
ssh samuel@192.168.0.38 "sudo chmod 755 /usr/local/bin/mcp_debug_server"

# Restart service
ssh samuel@192.168.0.38 "sudo systemctl restart mcp-debug.service"
```

**Issue: Environment File Missing**
```bash
# Check if file exists
ssh samuel@192.168.0.38 "ls -la /etc/mcp-debug/mcp-debug.env"

# If missing, re-transfer from local
rsync -avz /tmp/mcp-debug.env samuel@192.168.0.38:/tmp/
ssh samuel@192.168.0.38 "sudo mv /tmp/mcp-debug.env /etc/mcp-debug/"
ssh samuel@192.168.0.38 "sudo chown mcp:mcp /etc/mcp-debug/mcp-debug.env"
ssh samuel@192.168.0.38 "sudo chmod 600 /etc/mcp-debug/mcp-debug.env"

# Restart service
ssh samuel@192.168.0.38 "sudo systemctl restart mcp-debug.service"
```

### 2.2 Service Crashes Repeatedly

**Symptom**: Service starts but crashes within seconds/minutes

**Diagnosis**:
```bash
# Check restart count
ssh samuel@192.168.0.38 "systemctl show -p NRestarts mcp-debug.service"

# Check crash logs
ssh samuel@192.168.0.38 "sudo journalctl -u mcp-debug.service --since '1 hour ago' | grep -iE '(panic|fatal|abort|segfault)'"

# Check core dumps
ssh samuel@192.168.0.38 "ls -la /var/lib/systemd/coredump/ | grep mcp"
```

**Quick Fix: Reset Restart Counter**
```bash
ssh samuel@192.168.0.38 "sudo systemctl reset-failed mcp-debug.service"
ssh samuel@192.168.0.38 "sudo systemctl restart mcp-debug.service"
```

**If Still Failing**: Proceed to Full Rollback (Section 3)

### 2.3 High Resource Usage

**Symptom**: Memory >512MB or CPU >50%

**Diagnosis**:
```bash
ssh samuel@192.168.0.38 "systemctl status mcp-debug.service | grep -E '(Memory|CPU|Tasks)'"
```

**Quick Fix: Restart Service** (clears memory leaks):
```bash
ssh samuel@192.168.0.38 "sudo systemctl restart mcp-debug.service"
sleep 10
ssh samuel@192.168.0.38 "systemctl status mcp-debug.service | grep Memory"
```

**If Still High**: Adjust limits (Section 4) or Full Rollback

---

## 3. Full Rollback Procedure

**When to Use**: Service cannot be stabilized with quick fixes

**Estimated Time**: 5-10 minutes

**Impact**: Service will be stopped and removed

### 3.1 Pre-Rollback Backup

**Backup Current State** (for postmortem analysis):
```bash
# Backup logs
ssh samuel@192.168.0.38 "sudo journalctl -u mcp-debug.service > /tmp/mcp-debug-rollback-$(date +%Y%m%d-%H%M%S).log"

# Backup configuration
ssh samuel@192.168.0.38 "sudo cp /etc/mcp-debug/mcp-debug.env /opt/mcp-backups/mcp-debug.env.$(date +%Y%m%d-%H%M%S)"

# Backup service file
ssh samuel@192.168.0.38 "sudo cp /etc/systemd/system/mcp-debug.service /opt/mcp-backups/mcp-debug.service.$(date +%Y%m%d-%H%M%S)"

# Download backups to local machine
rsync -avz samuel@192.168.0.38:/tmp/mcp-debug-rollback-*.log /tmp/
rsync -avz samuel@192.168.0.38:/opt/mcp-backups/mcp-debug* /tmp/
```

### 3.2 Stop and Disable Service

```bash
# Stop service immediately
ssh samuel@192.168.0.38 "sudo systemctl stop mcp-debug.service"

# Verify stopped
ssh samuel@192.168.0.38 "systemctl is-active mcp-debug.service"
# Expected: inactive or failed

# Disable service (prevent auto-start on boot)
ssh samuel@192.168.0.38 "sudo systemctl disable mcp-debug.service"

# Verify disabled
ssh samuel@192.168.0.38 "systemctl is-enabled mcp-debug.service"
# Expected: disabled
```

### 3.3 Remove Service Files

```bash
# Remove systemd service file
ssh samuel@192.168.0.38 "sudo rm /etc/systemd/system/mcp-debug.service"

# Reload systemd daemon
ssh samuel@192.168.0.38 "sudo systemctl daemon-reload"

# Reset failed units
ssh samuel@192.168.0.38 "sudo systemctl reset-failed"
```

### 3.4 Remove Binary

```bash
# Remove binary
ssh samuel@192.168.0.38 "sudo rm /usr/local/bin/mcp_debug_server"

# Verify removal
ssh samuel@192.168.0.38 "ls /usr/local/bin/mcp_debug_server"
# Expected: No such file or directory
```

### 3.5 Clean Up State (Optional)

**WARNING**: This will delete all runtime state and logs

```bash
# Remove state directory
ssh samuel@192.168.0.38 "sudo rm -rf /var/lib/mcp"

# Remove log directory
ssh samuel@192.168.0.38 "sudo rm -rf /var/log/mcp"

# Remove runtime directory
ssh samuel@192.168.0.38 "sudo rm -rf /run/mcp"

# Remove cache directory
ssh samuel@192.168.0.38 "sudo rm -rf /var/cache/mcp"

# Remove shared memory
ssh samuel@192.168.0.38 "sudo rm -rf /dev/shm/mcp-shared"
```

### 3.6 Remove Configuration (Optional)

**WARNING**: Preserving config may be useful for postmortem

```bash
# Remove environment file (keep backup!)
ssh samuel@192.168.0.38 "sudo rm /etc/mcp-debug/mcp-debug.env"

# Remove config directory (if empty)
ssh samuel@192.168.0.38 "sudo rmdir /etc/mcp-debug"
```

### 3.7 Remove User (Optional)

**WARNING**: Only if completely removing mcp_debug_server

```bash
# Remove mcp user
ssh samuel@192.168.0.38 "sudo userdel mcp"

# Verify removal
ssh samuel@192.168.0.38 "id mcp"
# Expected: no such user
```

### 3.8 Verify Rollback Complete

```bash
# Service should not exist
ssh samuel@192.168.0.38 "systemctl status mcp-debug.service"
# Expected: Unit mcp-debug.service could not be found

# Binary should not exist
ssh samuel@192.168.0.38 "ls /usr/local/bin/mcp_debug_server"
# Expected: No such file or directory

# Port should be free
ssh samuel@192.168.0.38 "ss -tlnp | grep 5678"
# Expected: No output

# Process should not be running
ssh samuel@192.168.0.38 "ps aux | grep '[m]cp_debug_server'"
# Expected: No output
```

---

## 4. Partial Rollback (Adjust Limits)

**When to Use**: Service works but hits resource limits

### 4.1 Increase Memory Limit

**Current Limit**: 512MB (MemoryMax=512M)

**Procedure**:
```bash
# Stop service
ssh samuel@192.168.0.38 "sudo systemctl stop mcp-debug.service"

# Edit service file
ssh samuel@192.168.0.38 "sudo sed -i 's/MemoryMax=512M/MemoryMax=1G/' /etc/systemd/system/mcp-debug.service"
ssh samuel@192.168.0.38 "sudo sed -i 's/MemoryHigh=400M/MemoryHigh=800M/' /etc/systemd/system/mcp-debug.service"

# Reload systemd
ssh samuel@192.168.0.38 "sudo systemctl daemon-reload"

# Start service
ssh samuel@192.168.0.38 "sudo systemctl start mcp-debug.service"

# Verify new limits
ssh samuel@192.168.0.38 "systemctl show -p MemoryMax -p MemoryHigh mcp-debug.service"
# Expected: MemoryMax=1073741824 MemoryHigh=838860800
```

### 4.2 Increase CPU Limit

**Current Limit**: 50% (CPUQuota=50%)

**Procedure**:
```bash
# Stop service
ssh samuel@192.168.0.38 "sudo systemctl stop mcp-debug.service"

# Edit service file
ssh samuel@192.168.0.38 "sudo sed -i 's/CPUQuota=50%/CPUQuota=100%/' /etc/systemd/system/mcp-debug.service"

# Reload systemd
ssh samuel@192.168.0.38 "sudo systemctl daemon-reload"

# Start service
ssh samuel@192.168.0.38 "sudo systemctl start mcp-debug.service"

# Verify new limit
ssh samuel@192.168.0.38 "systemctl show -p CPUQuota mcp-debug.service"
# Expected: CPUQuota=1.000000
```

### 4.3 Increase File Descriptor Limit

**Current Limit**: 8192 (LimitNOFILE=8192)

**Procedure**:
```bash
# Stop service
ssh samuel@192.168.0.38 "sudo systemctl stop mcp-debug.service"

# Edit service file
ssh samuel@192.168.0.38 "sudo sed -i 's/LimitNOFILE=8192/LimitNOFILE=16384/' /etc/systemd/system/mcp-debug.service"

# Reload systemd
ssh samuel@192.168.0.38 "sudo systemctl daemon-reload"

# Start service
ssh samuel@192.168.0.38 "sudo systemctl start mcp-debug.service"

# Verify new limit
ssh samuel@192.168.0.38 "systemctl show -p LimitNOFILE mcp-debug.service"
# Expected: LimitNOFILE=16384
```

---

## 5. Restore Previous State

**If Previous MCP Server Was Running**:

### 5.1 Restore mcp_http_server (if existed)

**Check if Previous Server Existed**:
```bash
ssh samuel@192.168.0.38 "ls -la ~/mcp_http_server"
```

**Restore**:
```bash
# Start previous server
ssh samuel@192.168.0.38 "nohup ~/mcp_http_server &"

# Verify started
ssh samuel@192.168.0.38 "ps aux | grep '[m]cp_http_server'"

# Check port
ssh samuel@192.168.0.38 "ss -tlnp | grep 8080"
```

### 5.2 Restore Cloudflared Tunnel (if existed)

**Check if Cloudflared Was Running**:
```bash
ssh samuel@192.168.0.38 "ps aux | grep '[c]loudflared'"
```

**If Not Running**:
```bash
# Restart cloudflared (if it was stopped)
ssh samuel@192.168.0.38 "cloudflared tunnel --url http://localhost:8080 &"
```

---

## 6. Post-Rollback Verification

### 6.1 System Health Check

**Memory Freed**:
```bash
ssh samuel@192.168.0.38 "free -h"
# Expected: More free memory than before deployment
```

**Port Released**:
```bash
ssh samuel@192.168.0.38 "ss -tlnp | grep 5678"
# Expected: No output (port free)
```

**No Zombie Processes**:
```bash
ssh samuel@192.168.0.38 "ps aux | grep '[m]cp_debug'"
# Expected: No output
```

### 6.2 Previous State Restored

**Previous Services Running** (if applicable):
```bash
ssh samuel@192.168.0.38 "ps aux | grep -E '(mcp_http_server|cloudflared)' | grep -v grep"
# Expected: Previous services visible (if they were running before)
```

### 6.3 No Remnants

**No Orphaned Files**:
```bash
ssh samuel@192.168.0.38 "find /tmp -name '*mcp*' -mtime -1"
# Expected: Only backup files

ssh samuel@192.168.0.38 "find /var -name '*mcp*' 2>/dev/null"
# Expected: Only backups in /opt/mcp-backups (if preserved)
```

---

## 7. Postmortem Analysis

### 7.1 Collect Evidence

**Logs**:
```bash
# Already backed up in Section 3.1
# Review logs locally
less /tmp/mcp-debug-rollback-*.log
```

**System State at Failure**:
```bash
# Review resource usage
grep -iE '(memory|cpu|task)' /tmp/mcp-debug-rollback-*.log

# Review errors
grep -iE '(error|fatal|panic|abort)' /tmp/mcp-debug-rollback-*.log
```

**Core Dumps** (if any):
```bash
# Download core dumps
rsync -avz samuel@192.168.0.38:/var/lib/systemd/coredump/ /tmp/coredumps/

# Analyze with gdb (if core dump exists)
gdb /home/samuel/Primitives/target/release/mcp_debug_server /tmp/coredumps/core.*
```

### 7.2 Root Cause Analysis

**Common Failure Patterns**:

| Symptom | Likely Cause | Fix for Next Deployment |
|---------|--------------|-------------------------|
| OOM kill | Memory limit too low | Increase MemoryMax to 1GB |
| Port conflict | Another service using 5678 | Use different port (5679) |
| Permission denied | CAP_SYS_PTRACE not set | Verify setcap in deployment |
| Segfault | Binary corrupted | Re-build binary, verify SHA256 |
| Continuous crashes | Bug in code | Fix code, re-deploy |

### 7.3 Document Findings

**Template**:
```
# Rollback Postmortem - atomic_mcp_server
Date: YYYY-MM-DD HH:MM UTC
Deployed Version: v0.1.0
Rollback Reason: <reason>

## Timeline
- HH:MM - Deployment started
- HH:MM - Service started
- HH:MM - Issue detected (describe)
- HH:MM - Quick fix attempted (describe)
- HH:MM - Rollback initiated
- HH:MM - Rollback completed

## Root Cause
<Describe root cause>

## Impact
- Service downtime: <duration>
- Resources affected: <list>
- Data loss: <yes/no, describe>

## Resolution
<Describe how issue was resolved>

## Prevention
<Steps to prevent recurrence>

## Action Items
- [ ] Fix identified issue
- [ ] Update deployment plan
- [ ] Update validation procedures
- [ ] Re-test deployment in staging
```

---

## 8. Re-Deployment After Rollback

**Before Re-Deploying**:

- [ ] Root cause identified and fixed
- [ ] Code changes tested locally
- [ ] Deployment plan updated
- [ ] Validation procedures updated
- [ ] Memory pressure resolved (training_harness stopped)
- [ ] Port conflicts resolved
- [ ] Binary re-built with fixes
- [ ] SHA256 hash updated in deployment plan

**Re-Deployment Procedure**:
```bash
# Follow REMOTE_DEPLOYMENT_PLAN.md again
# Pay special attention to issues identified in postmortem
```

---

## 9. Emergency Contacts

**Escalation Path**:
1. Check logs (`journalctl -u mcp-debug.service`)
2. Try quick fixes (Section 2)
3. If not resolved in 15 minutes → Full rollback (Section 3)
4. Document in postmortem (Section 7)
5. Fix and re-deploy when ready (Section 8)

**Key Files**:
- Deployment plan: `/home/samuel/Primitives/atomic_mcp_server/REMOTE_DEPLOYMENT_PLAN.md`
- Validation: `/home/samuel/Primitives/atomic_mcp_server/DEPLOYMENT_VALIDATION.md`
- This file: `/home/samuel/Primitives/atomic_mcp_server/ROLLBACK_PROCEDURES.md`

---

**Generated**: 2025-11-19
**Purpose**: Emergency rollback for atomic_mcp_server deployment
**Critical**: Test rollback procedure in staging before production deployment
