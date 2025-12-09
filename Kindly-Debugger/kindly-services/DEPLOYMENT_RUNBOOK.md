# Kindly Services Deployment Runbook

**Version**: 1.0.0
**Date**: December 4, 2025
**Target**: kindly-hub (192.168.0.38)
**Framework**: UCE34/Chaos/T28/B32/ASSUM

---

## Table of Contents

1. [Pre-Deployment Checklist](#pre-deployment-checklist)
2. [Build Procedures](#build-procedures)
3. [Deployment Steps](#deployment-steps)
4. [Verification Procedures](#verification-procedures)
5. [Rollback Procedures](#rollback-procedures)
6. [Monitoring](#monitoring)
7. [Troubleshooting Guide](#troubleshooting-guide)
8. [Emergency Procedures](#emergency-procedures)

---

## Pre-Deployment Checklist

### Required Before ANY Deployment

- [ ] **Local Tests Pass**: Run `cargo test --features full-protection`
- [ ] **Build Succeeds**: Run `cargo build --release --bin http_server --features full-protection`
- [ ] **No Clippy Warnings**: Run `cargo clippy --features full-protection -- -D warnings`
- [ ] **lsyncd Active**: Verify sync is running (`journalctl --user -u lsyncd -n 5`)
- [ ] **Remote Accessible**: Test SSH to kindly-hub (`ssh samuel@kindly-hub "uptime"`)
- [ ] **Backup Current Binary**: Done automatically by deployment script

### Feature-Specific Checks

| Feature | Check | Command |
|---------|-------|---------|
| security-headers | Header injection works | Check SecurityHeadersCapsule tests |
| http-audit | Audit logging works | Check HttpAuditLogCapsule tests |
| rate-limiting | Rate limiter works | Check AdaptiveRateLimiterCapsule tests |

### Network Verification

```bash
# Verify kindly-hub is accessible
ssh samuel@kindly-hub "echo 'SSH OK'"

# Verify port 8082 is not blocked
ssh samuel@kindly-hub "ss -tlnp | grep 8082 || echo 'Port not in use'"

# Verify UFW allows port 8082
ssh samuel@kindly-hub "sudo ufw status | grep 8082"
```

---

## Build Procedures

### Standard Production Build

```bash
cd /home/samuel/Primitives/kindly-services

# Clean build directory (optional, for fresh build)
cargo clean

# Build with full protection
cargo build --release --bin http_server --features full-protection

# Verify binary exists
ls -la target/release/http_server
```

**Expected Output**:
- Binary size: ~400-500KB (stripped)
- Build time: ~30-60 seconds (fresh), ~5-10 seconds (incremental)

### Build Variants

| Variant | Command | Use Case |
|---------|---------|----------|
| **Development** | `cargo build --bin http_server` | Local testing, no protection |
| **Staging** | `cargo build --release --bin http_server --features security-headers` | Headers only |
| **Production** | `cargo build --release --bin http_server --features full-protection` | Full deployment |

### Cross-Compilation (if needed)

The target system (kindly-hub) is x86_64 Linux, same as build system. No cross-compilation needed.

---

## Deployment Steps

### Phase 1: Initial Deployment

**Step 1: Build Binary**
```bash
cd /home/samuel/Primitives/kindly-services
cargo build --release --bin http_server --features full-protection
```

**Step 2: Verify lsyncd Sync**
```bash
# Check lsyncd status
journalctl --user -u lsyncd -n 20

# Verify last sync (should be within 2-5 seconds)
# Look for: "Calling rsync..." entries
```

**Step 3: Deploy via lsyncd (Automatic)**
```bash
# lsyncd automatically syncs to kindly-hub
# Wait 2-5 seconds for sync to complete

# Verify binary arrived
ssh samuel@kindly-hub "ls -la ~/Primitives/kindly-services/target/release/http_server"
```

**Step 4: Stop Existing Service (if running)**
```bash
ssh samuel@kindly-hub "sudo systemctl stop kindly-services 2>/dev/null || pkill -f http_server || echo 'No service running'"
```

**Step 5: Start New Service**
```bash
ssh samuel@kindly-hub "cd ~/Primitives/kindly-services && nohup ./target/release/http_server > /tmp/kindly-services.log 2>&1 &"
```

**Step 6: Verify Startup**
```bash
# Check process running
ssh samuel@kindly-hub "pgrep -a http_server"

# Check listening on port
ssh samuel@kindly-hub "ss -tlnp | grep 8082"

# Check initial log
ssh samuel@kindly-hub "tail -20 /tmp/kindly-services.log"
```

### Phase 2: SystemD Service Deployment

**Step 1: Create Service File**
```bash
cat << 'EOF' | ssh samuel@kindly-hub "sudo tee /etc/systemd/system/kindly-services.service"
[Unit]
Description=Kindly Services HTTP Server
After=network.target

[Service]
Type=simple
User=samuel
Group=samuel
WorkingDirectory=/home/samuel/Primitives/kindly-services
ExecStart=/home/samuel/Primitives/kindly-services/target/release/http_server
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ReadOnlyPaths=/home/samuel/Primitives/kindly-services

# Environment
Environment=RUST_LOG=info
Environment=KINDLY_PORT=8082

[Install]
WantedBy=multi-user.target
EOF
```

**Step 2: Enable and Start Service**
```bash
ssh samuel@kindly-hub "sudo systemctl daemon-reload"
ssh samuel@kindly-hub "sudo systemctl enable kindly-services"
ssh samuel@kindly-hub "sudo systemctl start kindly-services"
```

**Step 3: Verify Service Status**
```bash
ssh samuel@kindly-hub "sudo systemctl status kindly-services"
```

---

## Verification Procedures

### Immediate Post-Deployment Checks

**Check 1: Service Running**
```bash
ssh samuel@kindly-hub "systemctl is-active kindly-services"
# Expected: active
```

**Check 2: Port Listening**
```bash
ssh samuel@kindly-hub "ss -tlnp | grep 8082"
# Expected: LISTEN  0  128  127.0.0.1:8082
```

**Check 3: HTTP Response**
```bash
ssh samuel@kindly-hub "curl -sI http://localhost:8082/ | head -5"
# Expected: HTTP/1.1 200 OK
```

**Check 4: Security Headers**
```bash
ssh samuel@kindly-hub "curl -sI http://localhost:8082/ | grep -E '^(Strict-Transport|X-Frame|X-Content)'"
# Expected: Security headers present
```

### Full Verification Script

```bash
# Run security tests
./scripts/security_test.sh --quick

# Run from remote
ssh samuel@kindly-hub "cd ~/Primitives/kindly-services && ./scripts/security_test.sh --local"
```

### Health Check Endpoints

| Endpoint | Expected | Meaning |
|----------|----------|---------|
| `GET /` | 200 OK | Service operational |
| `GET /index.html` | 200 OK | Static serving works |
| `GET /../etc/passwd` | 403 | Path security works |
| `GET /nonexistent` | 200 (SPA) | SPA fallback works |

---

## Rollback Procedures

### Quick Rollback (< 1 minute)

If deployment fails, immediately rollback:

```bash
# Stop broken service
ssh samuel@kindly-hub "sudo systemctl stop kindly-services"

# Restore from backup (if exists)
ssh samuel@kindly-hub "
if [ -f ~/Primitives/kindly-services/target/release/http_server.backup ]; then
    mv ~/Primitives/kindly-services/target/release/http_server.backup \
       ~/Primitives/kindly-services/target/release/http_server
    sudo systemctl start kindly-services
    echo 'Rollback complete'
else
    echo 'No backup found!'
fi
"
```

### Feature Rollback

If a specific feature causes issues:

```bash
# Rebuild without problematic feature
cd /home/samuel/Primitives/kindly-services

# Example: Disable rate limiting but keep headers
cargo build --release --bin http_server --features "security-headers,http-audit"

# Wait for lsyncd sync (2-5 seconds)
sleep 5

# Restart service
ssh samuel@kindly-hub "sudo systemctl restart kindly-services"
```

### Full Rollback to Known-Good Version

```bash
# Checkout last known-good commit
cd /home/samuel/Primitives/kindly-services
git log --oneline -5  # Find last good commit
git checkout <commit-hash> -- src/bin/http_server.rs Cargo.toml

# Rebuild
cargo build --release --bin http_server --features full-protection

# Deploy
sleep 5  # Wait for lsyncd
ssh samuel@kindly-hub "sudo systemctl restart kindly-services"
```

### Nuclear Rollback (Last Resort)

Disable all protection and run minimal server:

```bash
# Build without any protection
cargo build --release --bin http_server

# Deploy and restart
sleep 5
ssh samuel@kindly-hub "sudo systemctl restart kindly-services"
```

---

## Monitoring

### Log Locations

| Log | Location | Command |
|-----|----------|---------|
| Service logs | journald | `ssh samuel@kindly-hub "journalctl -u kindly-services -f"` |
| Audit logs | stdout | Included in journald |
| fail2ban | /var/log/fail2ban.log | `ssh samuel@kindly-hub "sudo tail -f /var/log/fail2ban.log"` |
| UFW | /var/log/ufw.log | `ssh samuel@kindly-hub "sudo tail -f /var/log/ufw.log"` |

### Key Metrics to Monitor

| Metric | Source | Alert Threshold |
|--------|--------|-----------------|
| HTTP 5xx rate | journald [AUDIT] | >1% of requests |
| 429 rate | journald [AUDIT] | >10% of requests (sustained) |
| Response time | journald [AUDIT] | >1 second average |
| Process restarts | journald | >3 in 10 minutes |
| Memory usage | `ps` | >100MB |

### Monitoring Commands

```bash
# Watch live logs
ssh samuel@kindly-hub "journalctl -u kindly-services -f"

# Count requests by status (last 100 entries)
ssh samuel@kindly-hub "journalctl -u kindly-services -n 100 | grep '\[AUDIT\]' | awk '{print \$6}' | sort | uniq -c"

# Check memory usage
ssh samuel@kindly-hub "ps aux | grep http_server"

# Check CPU usage
ssh samuel@kindly-hub "top -bn1 | grep http_server"
```

### Alerting (Manual)

Set up simple monitoring cron:

```bash
# Add to kindly-hub crontab
ssh samuel@kindly-hub "crontab -l"

# Add health check
(crontab -l 2>/dev/null; echo "*/5 * * * * curl -sf http://localhost:8082/ > /dev/null || echo 'Kindly Services DOWN' | mail -s 'ALERT' you@example.com") | crontab -
```

---

## Troubleshooting Guide

### Common Issues

#### Issue: Service Won't Start

**Symptoms**: `systemctl status kindly-services` shows "failed"

**Diagnosis**:
```bash
ssh samuel@kindly-hub "journalctl -u kindly-services -n 50 --no-pager"
```

**Common Causes**:
| Error | Cause | Fix |
|-------|-------|-----|
| "Address in use" | Port 8082 occupied | `sudo fuser -k 8082/tcp` |
| "Permission denied" | Binary not executable | `chmod +x target/release/http_server` |
| "No such file" | Binary not synced | Wait for lsyncd, or copy manually |
| "dist not found" | dist directory missing | Run `trunk build --release` |

#### Issue: 403 on All Requests

**Symptoms**: Every request returns 403 Forbidden

**Diagnosis**:
```bash
# Check if PathValidator is rejecting
ssh samuel@kindly-hub "journalctl -u kindly-services -n 20 | grep SECURITY"
```

**Fix**: Check path validation logic, may be too aggressive

#### Issue: No Security Headers

**Symptoms**: `curl -I` shows no HSTS, X-Frame-Options, etc.

**Diagnosis**:
```bash
# Check if feature enabled
cargo metadata --format-version=1 | jq '.packages[] | select(.name=="kindly-services") | .features'
```

**Fix**: Ensure `--features security-headers` was used

#### Issue: Rate Limiting Too Aggressive

**Symptoms**: Legitimate traffic getting 429

**Fix**: Adjust rate limiter configuration:
```rust
// In http_server.rs, increase limits:
static ref RATE_LIMITER: AdaptiveRateLimiterCapsule =
    AdaptiveRateLimiterCapsule::new(1000, 200); // Higher limits
```

#### Issue: High Memory Usage

**Symptoms**: Process using >100MB RAM

**Diagnosis**:
```bash
ssh samuel@kindly-hub "ps -o rss= -p \$(pgrep http_server)"
```

**Fix**: Check audit log ring buffer size, may need restart

### Diagnostic Commands

```bash
# Full system status
ssh samuel@kindly-hub "
echo '=== Process ==='
ps aux | grep http_server
echo '=== Port ==='
ss -tlnp | grep 8082
echo '=== Memory ==='
free -h
echo '=== Disk ==='
df -h /home/samuel
echo '=== UFW ==='
sudo ufw status
echo '=== fail2ban ==='
sudo fail2ban-client status
"
```

---

## Emergency Procedures

### Service Completely Down

**Immediate Actions**:
```bash
# 1. Check if process exists
ssh samuel@kindly-hub "pgrep http_server"

# 2. Try restart
ssh samuel@kindly-hub "sudo systemctl restart kindly-services"

# 3. If fails, check logs
ssh samuel@kindly-hub "journalctl -u kindly-services -n 100 --no-pager"

# 4. Manual start for debugging
ssh samuel@kindly-hub "
cd ~/Primitives/kindly-services
./target/release/http_server 2>&1 | head -50
"
```

### Under Attack (High Traffic)

**Immediate Actions**:
```bash
# 1. Check request rate
ssh samuel@kindly-hub "journalctl -u kindly-services --since '5 minutes ago' | grep '\[AUDIT\]' | wc -l"

# 2. Identify attacking IPs (if logged)
ssh samuel@kindly-hub "journalctl -u kindly-services --since '5 minutes ago' | grep '\[AUDIT\]' | grep '429' | head -20"

# 3. Block at firewall level
ssh samuel@kindly-hub "sudo ufw deny from 1.2.3.4"  # Replace with attacker IP

# 4. Increase rate limit temporarily
# (requires rebuild - see Rate Limiting Too Aggressive)
```

### Data Corruption / Tampering Detected

**Immediate Actions**:
```bash
# 1. Check audit trail integrity
ssh samuel@kindly-hub "journalctl -u kindly-services | grep 'TAMPER'"

# 2. Stop service to preserve state
ssh samuel@kindly-hub "sudo systemctl stop kindly-services"

# 3. Preserve logs for forensics
ssh samuel@kindly-hub "journalctl -u kindly-services > /tmp/kindly-forensics-\$(date +%Y%m%d-%H%M%S).log"

# 4. Investigate before restart
```

### SSH Lockout

If you're locked out of kindly-hub:

1. **Check from another machine** if available
2. **Physical access** to kindly-hub (console login)
3. **Network admin access** to router/firewall
4. **fail2ban unban** (if that's the cause): `fail2ban-client set sshd unbanip YOUR_IP`

### Contact Escalation

1. **L1**: Check logs, restart service
2. **L2**: Rebuild without problematic features
3. **L3**: Full rollback to known-good version
4. **L4**: Nuclear rollback (minimal server)

---

## Appendix A: Deployment Script

Save as `scripts/deploy.sh`:

```bash
#!/bin/bash
set -euo pipefail

echo "=== Kindly Services Deployment ==="
echo "Date: $(date)"

# Build
echo ""
echo "Step 1: Building..."
cargo build --release --bin http_server --features full-protection

# Wait for lsyncd
echo ""
echo "Step 2: Waiting for lsyncd sync..."
sleep 5

# Backup existing binary
echo ""
echo "Step 3: Backing up existing binary..."
ssh samuel@kindly-hub "
if [ -f ~/Primitives/kindly-services/target/release/http_server ]; then
    cp ~/Primitives/kindly-services/target/release/http_server \
       ~/Primitives/kindly-services/target/release/http_server.backup
    echo 'Backup created'
fi
"

# Restart service
echo ""
echo "Step 4: Restarting service..."
ssh samuel@kindly-hub "sudo systemctl restart kindly-services"

# Verify
echo ""
echo "Step 5: Verifying..."
sleep 2
ssh samuel@kindly-hub "systemctl is-active kindly-services"
ssh samuel@kindly-hub "curl -sI http://localhost:8082/ | head -3"

echo ""
echo "=== Deployment Complete ==="
```

---

## Appendix B: Quick Reference Card

```
=== KINDLY SERVICES DEPLOYMENT ===

BUILD:
  cargo build --release --bin http_server --features full-protection

DEPLOY:
  Wait for lsyncd (2-5s)
  ssh samuel@kindly-hub "sudo systemctl restart kindly-services"

VERIFY:
  ssh samuel@kindly-hub "curl -sI http://localhost:8082/ | head -5"
  ./scripts/security_test.sh --quick

LOGS:
  ssh samuel@kindly-hub "journalctl -u kindly-services -f"

ROLLBACK:
  ssh samuel@kindly-hub "sudo systemctl stop kindly-services"
  ssh samuel@kindly-hub "mv ~/Primitives/.../http_server.backup ~/Primitives/.../http_server"
  ssh samuel@kindly-hub "sudo systemctl start kindly-services"

EMERGENCY:
  ssh samuel@kindly-hub "sudo systemctl stop kindly-services"
  ssh samuel@kindly-hub "pkill -9 http_server"
```

---

**Document Status**: Complete
**Last Updated**: December 4, 2025
**Review**: Pending Operations Team Review
