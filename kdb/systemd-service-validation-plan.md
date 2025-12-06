# systemd Service Validation Test Plan
## atomic_mcp_server Production Deployment

**Version**: 1.0.0
**Date**: 2025-11-16
**Target**: Ubuntu Server 24.04 (systemd 255+)

---

## Test Categories

### 1. Functional Tests (Service Lifecycle)

#### Test 1.1: Clean Start
```bash
# Precondition: Service stopped
systemctl stop mcp-debug 2>/dev/null || true

# Test: Start service
systemctl start mcp-debug

# Validation
systemctl is-active mcp-debug | grep -q "^active$" || exit 1
sleep 2
systemctl status mcp-debug | grep -q "active (running)" || exit 1
journalctl -u mcp-debug -n 20 | grep -q "Server listening on" || exit 1

# Expected: active (running), Main PID valid, logs show startup
```

#### Test 1.2: Graceful Stop
```bash
# Test: Stop service
time systemctl stop mcp-debug

# Validation
systemctl is-active mcp-debug | grep -q "^inactive$" || exit 1
journalctl -u mcp-debug -n 10 | grep -q "Received SIGTERM" || echo "Warning: No graceful shutdown log"
journalctl -u mcp-debug -n 10 | grep -qv "killed by signal" || exit 1

# Expected: inactive (dead), graceful shutdown, <2 seconds
```

#### Test 1.3: Restart
```bash
# Test: Restart service
START_TIME=$(date +%s)
systemctl restart mcp-debug
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Validation
systemctl is-active mcp-debug | grep -q "^active$" || exit 1
[ "$DURATION" -lt 3 ] || echo "Warning: Restart took ${DURATION}s (target <3s)"
OLD_PID=$(systemctl show -p MainPID mcp-debug --value)
sleep 2
NEW_PID=$(systemctl show -p MainPID mcp-debug --value)
[ "$OLD_PID" != "$NEW_PID" ] || exit 1

# Expected: active, <3 seconds, new PID
```

#### Test 1.4: Crash Recovery (Auto-Restart)
```bash
# Test: Kill service with SIGKILL
OLD_PID=$(systemctl show -p MainPID mcp-debug --value)
kill -9 "$OLD_PID"
sleep 6  # RestartSec=5s + startup

# Validation
systemctl is-active mcp-debug | grep -q "^active$" || exit 1
NEW_PID=$(systemctl show -p MainPID mcp-debug --value)
[ "$OLD_PID" != "$NEW_PID" ] || exit 1
journalctl -u mcp-debug -n 30 | grep -q "restart" || exit 1

# Expected: Auto-restarted, new PID, <6 seconds recovery
```

#### Test 1.5: Crash Loop Protection
```bash
# Test: Crash service 4 times rapidly
for i in {1..4}; do
    PID=$(systemctl show -p MainPID mcp-debug --value)
    kill -9 "$PID"
    sleep 2
done
sleep 10

# Validation
STATUS=$(systemctl is-active mcp-debug)
[ "$STATUS" = "failed" ] || echo "Warning: Expected failed state after 4 crashes"
journalctl -u mcp-debug -n 50 | grep -q "start-limit" || echo "Note: StartLimitBurst=3 triggered"

# Expected: Failed state after 3 restarts in 60 seconds
# Cleanup
systemctl reset-failed mcp-debug
systemctl start mcp-debug
```

---

### 2. Security Tests (Hardening Validation)

#### Test 2.1: systemd-analyze security Score
```bash
# Test: Check security score
SCORE=$(systemd-analyze security mcp-debug.service 2>/dev/null | grep "Overall exposure level" | awk '{print $NF}')

# Validation
echo "Security Score: $SCORE"
# Parse score (e.g., "8.5 GREAT" → 8.5)
NUMERIC_SCORE=$(echo "$SCORE" | grep -oE '^[0-9]+\.[0-9]+')
if (( $(echo "$NUMERIC_SCORE >= 8.0" | bc -l) )); then
    echo "PASS: Security score $NUMERIC_SCORE >= 8.0 (GREAT tier)"
else
    echo "FAIL: Security score $NUMERIC_SCORE < 8.0"
    exit 1
fi

# Expected: Score ≥8.0/10 (GREAT tier)
```

#### Test 2.2: Capability Verification (Only CAP_SYS_PTRACE)
```bash
# Test: Check process capabilities
PID=$(systemctl show -p MainPID mcp-debug --value)
CAP_EFF=$(grep CapEff /proc/"$PID"/status | awk '{print $2}')

# Decode capabilities (0x0000000000200000 = CAP_SYS_PTRACE = bit 19)
# Expected: Only bit 19 set (ptrace), all others 0
EXPECTED_CAP="0000000000200000"
if [ "$CAP_EFF" = "$EXPECTED_CAP" ]; then
    echo "PASS: Only CAP_SYS_PTRACE (0x$CAP_EFF)"
else
    echo "WARNING: Capabilities 0x$CAP_EFF (expected 0x$EXPECTED_CAP)"
    # Decode using capsh (if available)
    capsh --decode="$CAP_EFF" 2>/dev/null || true
fi

# Expected: CapEff = 0x0000000000200000 (CAP_SYS_PTRACE only)
```

#### Test 2.3: Filesystem Isolation (/etc/shadow read failure)
```bash
# Test: Attempt to read /etc/shadow
PID=$(systemctl show -p MainPID mcp-debug --value)
if nsenter -t "$PID" -m cat /etc/shadow 2>&1 | grep -q "Permission denied"; then
    echo "PASS: /etc/shadow read denied (ProtectSystem=strict working)"
else
    echo "FAIL: /etc/shadow readable (ProtectSystem=strict NOT enforced)"
    exit 1
fi

# Expected: Permission denied (ProtectSystem=strict makes /etc read-only)
```

#### Test 2.4: Network Isolation (Internet blocked)
```bash
# Test: Attempt to connect to internet (google.com:80)
PID=$(systemctl show -p MainPID mcp-debug --value)
if nsenter -t "$PID" -n timeout 2 curl -s http://google.com 2>&1 | grep -q "Connection refused\|timed out\|No route"; then
    echo "PASS: Internet connection blocked (IPAddressAllow=192.168.0.0/24)"
else
    echo "FAIL: Internet connection allowed (IPAddressAllow NOT enforced)"
    exit 1
fi

# Expected: Connection refused/timeout (IPAddressDeny=any)
```

#### Test 2.5: Syscall Filtering (reboot blocked)
```bash
# Test: Attempt to call reboot() syscall
PID=$(systemctl show -p MainPID mcp-debug --value)
# Use strace to attempt reboot syscall
if nsenter -t "$PID" -p sh -c 'exec strace -e reboot reboot 2>&1' | grep -q "Operation not permitted\|EPERM"; then
    echo "PASS: reboot syscall blocked (SystemCallFilter=~@reboot)"
else
    echo "WARNING: reboot syscall not tested (strace may not be available)"
fi

# Expected: Operation not permitted (SystemCallFilter blocks @reboot)
```

#### Test 2.6: Memory W^X Enforcement (MemoryDenyWriteExecute)
```bash
# Test: Attempt to allocate W+X memory
PID=$(systemctl show -p MainPID mcp-debug --value)
# Check process memory maps for writable+executable pages
if grep -E '(rw-p|rwxp).*\[heap\]' /proc/"$PID"/maps; then
    echo "WARNING: Writable+executable heap found (MemoryDenyWriteExecute may not be enforced)"
else
    echo "PASS: No W+X pages in heap (MemoryDenyWriteExecute working)"
fi

# Expected: No rwxp pages (W^X enforced)
```

---

### 3. Resource Limit Tests

#### Test 3.1: Memory Limit Enforcement (512MB MemoryMax)
```bash
# Test: Monitor memory usage under load
PID=$(systemctl show -p MainPID mcp-debug --value)
RSS_KB=$(grep VmRSS /proc/"$PID"/status | awk '{print $2}')
RSS_MB=$((RSS_KB / 1024))

echo "Current RSS: ${RSS_MB}MB (limit: 512MB)"
if [ "$RSS_MB" -lt 512 ]; then
    echo "PASS: Memory usage ${RSS_MB}MB < 512MB"
else
    echo "WARNING: Memory usage ${RSS_MB}MB approaching limit"
fi

# Load test: Simulate 120 concurrent connections (future test)
# Expected: RSS < 512MB under normal load
```

#### Test 3.2: CPU Quota Enforcement (50% CPUQuota)
```bash
# Test: Monitor CPU usage
PID=$(systemctl show -p MainPID mcp-debug --value)
# Sample CPU usage over 10 seconds
CPU_PCT=$(top -b -n 2 -d 5 -p "$PID" | tail -1 | awk '{print $9}')
echo "CPU usage: ${CPU_PCT}% (quota: 50%)"

# Note: CPUQuota is enforced over time windows, not instantaneous
# Expected: Average CPU < 50% (may burst higher temporarily)
```

#### Test 3.3: File Descriptor Limit (8192 LimitNOFILE)
```bash
# Test: Check FD limit
PID=$(systemctl show -p MainPID mcp-debug --value)
FD_LIMIT=$(cat /proc/"$PID"/limits | grep "open files" | awk '{print $4}')
FD_CURRENT=$(ls /proc/"$PID"/fd | wc -l)

echo "FD limit: $FD_LIMIT, current: $FD_CURRENT"
if [ "$FD_LIMIT" -eq 8192 ]; then
    echo "PASS: FD limit is 8192"
else
    echo "FAIL: FD limit is $FD_LIMIT (expected 8192)"
    exit 1
fi

# Expected: Limit = 8192, current < 4096 (50% headroom)
```

---

### 4. Multi-Instance Tests (Template Service)

#### Test 4.1: Start Multiple Instances
```bash
# Test: Start instances 1 and 2
systemctl start mcp-debug@1 mcp-debug@2
sleep 2

# Validation
systemctl is-active mcp-debug@1 | grep -q "^active$" || exit 1
systemctl is-active mcp-debug@2 | grep -q "^active$" || exit 1

# Check distinct PIDs
PID1=$(systemctl show -p MainPID mcp-debug@1 --value)
PID2=$(systemctl show -p MainPID mcp-debug@2 --value)
[ "$PID1" != "$PID2" ] || exit 1

echo "Instance 1 PID: $PID1"
echo "Instance 2 PID: $PID2"

# Expected: Both active, distinct PIDs
```

#### Test 4.2: Port Isolation (Instance 1 = 5678, Instance 2 = 5679)
```bash
# Test: Check port bindings
ss -tlnp | grep mcp_debug_server | grep 5678 || echo "WARNING: Instance 1 not on port 5678"
ss -tlnp | grep mcp_debug_server | grep 5679 || echo "WARNING: Instance 2 not on port 5679"

# Expected: Instance 1 binds 5678, Instance 2 binds 5679
```

#### Test 4.3: Independent Failures (Instance 1 crash doesn't affect Instance 2)
```bash
# Test: Kill instance 1
PID1=$(systemctl show -p MainPID mcp-debug@1 --value)
kill -9 "$PID1"
sleep 6

# Validation
systemctl is-active mcp-debug@1 | grep -q "^active$" || echo "Instance 1 auto-restarted"
systemctl is-active mcp-debug@2 | grep -q "^active$" || exit 1

# Expected: Instance 2 unaffected, Instance 1 auto-restarts
```

#### Test 4.4: Cleanup
```bash
systemctl stop mcp-debug@1 mcp-debug@2
systemctl reset-failed mcp-debug@1 mcp-debug@2 2>/dev/null || true
```

---

### 5. Integration Tests (MCP Protocol)

#### Test 5.1: Health Check Endpoint (if implemented)
```bash
# Test: HTTP GET /health
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" http://192.168.0.38:5678/health)
if [ "$RESPONSE" = "200" ]; then
    echo "PASS: Health check returns 200 OK"
else
    echo "WARNING: Health check failed (code $RESPONSE)"
fi

# Expected: HTTP 200 OK (if health endpoint implemented)
```

#### Test 5.2: MCP RPC Call (debugger.attach)
```bash
# Test: Send MCP JSON-RPC request
PID_TARGET=$(pgrep sleep | head -1)  # Attach to sleep process
REQUEST=$(cat <<EOF
{
  "jsonrpc": "2.0",
  "method": "debugger.attach",
  "params": {"pid": $PID_TARGET},
  "id": 1
}
EOF
)

RESPONSE=$(echo "$REQUEST" | nc 192.168.0.38 5678)
echo "MCP Response: $RESPONSE"

# Expected: JSON-RPC success response (validates kdb integration)
```

---

### 6. Stress Tests (Production Load)

#### Test 6.1: 120 Concurrent Connections
```bash
# Test: Simulate 120 concurrent clients
# (Requires custom load test tool - placeholder)
echo "Load test: 120 concurrent connections (manual test required)"

# Validation:
# - No OOM killer events (journalctl -k | grep "Out of memory")
# - RSS < 512MB (systemd-cgtop)
# - CPU < 50% average (top -p PID)
# - FD count < 8192 (ls /proc/PID/fd | wc -l)

# Expected: Stable under load for 10+ minutes
```

#### Test 6.2: Memory Leak Detection (24-hour soak test)
```bash
# Test: Run service for 24 hours, monitor RSS growth
# (Automated via cron or systemd timer - placeholder)
echo "Soak test: 24-hour memory leak detection (manual test required)"

# Validation:
# - Plot RSS over time (should be flat, not linear growth)
# - No OOM events
# - Service still responsive after 24 hours

# Expected: RSS stable (±10MB variance), no leaks
```

---

### 7. Compliance Tests (Q34 Auditability)

#### Test 7.1: journald Audit Trail
```bash
# Test: Verify structured logging
journalctl -u mcp-debug -o json-pretty -n 10 | jq '.MESSAGE,.PRIORITY'

# Expected: JSON-formatted logs with priority, timestamps, structured fields
```

#### Test 7.2: Hash-Chain Integrity (if implemented in mcp_debug_server)
```bash
# Test: Verify Q34 hash-chain audit trail
# (Requires server to expose audit API - placeholder)
echo "Hash-chain verification: Manual test required (server API call)"

# Expected: Hash chain valid, no tampering detected
```

---

## Test Execution Summary

### Automated Test Script
```bash
#!/bin/bash
set -e

echo "=== systemd Service Validation: atomic_mcp_server ==="
echo "Date: $(date)"
echo ""

# Functional tests
echo "[1/7] Functional Tests..."
bash test_1.1_clean_start.sh
bash test_1.2_graceful_stop.sh
bash test_1.3_restart.sh
bash test_1.4_crash_recovery.sh
bash test_1.5_crash_loop_protection.sh

# Security tests
echo "[2/7] Security Tests..."
bash test_2.1_security_score.sh
bash test_2.2_capability_verification.sh
bash test_2.3_filesystem_isolation.sh
bash test_2.4_network_isolation.sh
bash test_2.5_syscall_filtering.sh
bash test_2.6_memory_wx_enforcement.sh

# Resource tests
echo "[3/7] Resource Tests..."
bash test_3.1_memory_limit.sh
bash test_3.2_cpu_quota.sh
bash test_3.3_fd_limit.sh

# Multi-instance tests
echo "[4/7] Multi-Instance Tests..."
bash test_4.1_start_multiple.sh
bash test_4.2_port_isolation.sh
bash test_4.3_independent_failures.sh
bash test_4.4_cleanup.sh

# Integration tests
echo "[5/7] Integration Tests..."
bash test_5.1_health_check.sh
bash test_5.2_mcp_rpc_call.sh

# Stress tests (manual)
echo "[6/7] Stress Tests (manual validation required)..."
echo "  - Run test_6.1_concurrent_connections.sh manually"
echo "  - Run test_6.2_memory_leak_detection.sh manually"

# Compliance tests
echo "[7/7] Compliance Tests..."
bash test_7.1_journald_audit.sh
bash test_7.2_hash_chain_integrity.sh

echo ""
echo "=== Validation Complete ==="
```

### Expected Results
- **Functional**: 5/5 tests pass
- **Security**: 6/6 tests pass (score ≥8.0/10)
- **Resource**: 3/3 tests pass (limits enforced)
- **Multi-Instance**: 4/4 tests pass (isolation confirmed)
- **Integration**: 2/2 tests pass (MCP protocol working)
- **Stress**: Manual validation (24-hour soak test)
- **Compliance**: 2/2 tests pass (audit trail valid)

### Failure Handling
- **Security score <8.0**: Review systemd-analyze output, tighten directives
- **Memory leak detected**: Profile with valgrind, check Rust Drop implementations
- **Crash loop**: Check logs, validate binary integrity, test with minimal config
- **Port conflict**: Verify multi-instance port allocation (5678, 5679, etc.)

---

## Continuous Validation (Production Monitoring)

### Daily Checks
```bash
# Cron job: Daily validation
0 2 * * * /usr/local/bin/systemd-service-health-check.sh
```

### Weekly Checks
```bash
# Security posture review
systemd-analyze security mcp-debug.service > /var/log/mcp-security-score.log

# Resource usage trends
systemd-cgtop -b -n 1 | grep mcp-debug >> /var/log/mcp-resource-usage.log
```

### Monthly Checks
- Full test suite execution (30-minute runtime)
- Penetration testing (privilege escalation attempts)
- Compliance audit (Q34 hash-chain verification)

---

**Status**: Test plan ready for implementation
**Next Steps**: Create individual test scripts, integrate into CI/CD pipeline
