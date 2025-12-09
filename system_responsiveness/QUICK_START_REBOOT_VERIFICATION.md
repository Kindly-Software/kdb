# Quick Start: Reboot Persistence Verification

**Purpose**: Fast verification that daemon auto-starts after system reboot
**Time Required**: <5 minutes
**When to Run**: After any system reboot

---

## Pre-Reboot Checklist (Run Now)

```bash
# 1. Verify linger is enabled
loginctl show-user samuel | grep Linger
# Expected output: Linger=yes ✅

# 2. Verify service is enabled
systemctl --user is-enabled sysrespond.service
# Expected output: enabled ✅

# 3. Verify service is running now
systemctl --user is-active sysrespond.service
# Expected output: active ✅
```

**If all three show expected output**: Ready for reboot verification ✅

---

## Post-Reboot Verification (After Next Reboot)

### Immediate Check (Do this before logging in if possible)

```bash
# If you can SSH in without GUI login:
systemctl --user status sysrespond.service

# Expected output should show:
#   Loaded: loaded (.../sysrespond.service; enabled; preset: enabled)
#   Active: active (running) since [TODAY'S DATE]

# If you see this, daemon auto-started! ✅
```

### Desktop Login Check

```bash
# After normal desktop login, run:
systemctl --user status sysrespond.service

# Verify it shows:
#   Active: active (running) since [TIME ~60s after boot]
#   Memory: 25-30M (normal range)
#   CPU: <1min (just started)
```

### Detailed Verification

```bash
# Check when it started (should be ~60 seconds after boot)
systemctl --user show -p ActiveEnterTimestamp sysrespond.service

# View startup log entries
journalctl --user -u sysrespond.service | grep "System Responsiveness"
# Should show: "🚀 System Responsiveness Daemon v0.1.0"

# View memory usage trend
systemctl --user status sysrespond.service | grep Memory
# Should show: ~26M (within limits)

# View circuit breaker status
journalctl --user -u sysrespond.service | tail -1
# Should show: "Circuit breaker reset: state=Closed"
```

---

## Functional Test (Optional)

### Test 1: Verify Detection Works

```bash
# Create a hung test process
while true; do :; done &
TESTPID=$!
echo "Created test PID: $TESTPID"

# Wait 5+ minutes (threshold is 300 seconds)
echo "Waiting 320 seconds..."
sleep 320

# Check if killed
ps aux | grep "$TESTPID" | grep -v grep
# Expected: No output (process was killed)

echo "Test successful: Process was killed!" ✅
```

### Test 2: Verify Logging Works

```bash
# Check that kill was logged
journalctl --user -u sysrespond.service | grep "Killing hung"
# Should show your test process was killed

# Verify circuit breaker tracked it
journalctl --user -u sysrespond.service | grep "Circuit breaker"
# Should show current kill count
```

### Test 3: Verify Whitelisting Works

```bash
# Create a whitelisted process (like 'claude' or 'firefox')
# It should NOT be killed, even if hung

# Example with gnome-shell (whitelisted):
# gnome-shell runs long, should not be killed
pgrep -l gnome-shell
# Should still exist after 5 minutes
```

---

## Troubleshooting Quick Reference

### Problem: Service not running after reboot

```bash
# Step 1: Check if linger is still enabled
loginctl show-user samuel | grep Linger
# Must show: Linger=yes

# Step 2: Check if service is still enabled
systemctl --user is-enabled sysrespond.service
# Must show: enabled

# Step 3: Check for startup errors
journalctl --user -u sysrespond.service | head -20

# Step 4: Try manual start
systemctl --user start sysrespond.service

# Step 5: Check status
systemctl --user status sysrespond.service
```

### Problem: Memory usage is too high

```bash
# Check current memory
systemctl --user status sysrespond.service | grep Memory

# Check memory limit
systemctl --user status sysrespond.service | grep Max
# Should not exceed 50M (MemoryMax=50M)

# If high, restart daemon
systemctl --user restart sysrespond.service

# If persists, check logs
journalctl --user -u sysrespond.service -f | grep Memory
```

### Problem: Too many process kills

```bash
# Check circuit breaker status
journalctl --user -u sysrespond.service | grep "Circuit breaker"

# If OPEN (disabled), it detected too many kills
# Check configuration thresholds
cat ~/.config/sysrespond/config.toml | grep -A5 "\[thresholds\]"

# If too aggressive, adjust:
vi ~/.config/sysrespond/config.toml
# Increase cpu_threshold_pct (e.g., 150.0 instead of 100.0)
# Or increase runtime_threshold_sec (e.g., 600 instead of 300)

# Restart after changes
systemctl --user restart sysrespond.service
```

---

## Key Commands Reference

### Status & Monitoring

```bash
# Current service status
systemctl --user status sysrespond.service

# Check if running
systemctl --user is-active sysrespond.service

# Check if enabled
systemctl --user is-enabled sysrespond.service

# View linger status
loginctl show-user samuel | grep Linger
```

### Control Service

```bash
# Start service
systemctl --user start sysrespond.service

# Stop service
systemctl --user stop sysrespond.service

# Restart service
systemctl --user restart sysrespond.service

# Enable service
systemctl --user enable sysrespond.service

# Disable service
systemctl --user disable sysrespond.service
```

### Logging

```bash
# View recent logs (last 50 lines)
journalctl --user -u sysrespond.service -n 50

# Follow logs in real-time
journalctl --user -u sysrespond.service -f

# View logs since last reboot
journalctl --user -u sysrespond.service -b

# View logs from specific time
journalctl --user -u sysrespond.service --since "2 hours ago"

# View logs with timestamps
journalctl --user -u sysrespond.service -o verbose
```

### Configuration

```bash
# View current configuration
cat ~/.config/sysrespond/config.toml

# Edit configuration
vi ~/.config/sysrespond/config.toml

# After editing, restart daemon
systemctl --user restart sysrespond.service
```

---

## Expected Behavior Timeline

### On System Boot (with linger enabled)

| Time | Event |
|------|-------|
| t=0s | System power-on |
| t=20-30s | Kernel loads, systemd starts |
| t=30-35s | System services initialize |
| t=35-40s | user@1000 session created (auto-start due to linger) |
| t=40-45s | sysrespond.service starts |
| t=45-50s | First process scan begins |
| t=5+ min | Hung processes detected and killed (if any) |

**Key Point**: Daemon is protecting system before user login (~40-45s after boot)

### During Normal Operation

```bash
Every 10 seconds:
  - Scan all processes
  - Check CPU usage
  - Check runtime
  - Detect hung processes
  - Send SIGTERM to hung processes
  - Track in circuit breaker

Every 30 seconds:
  - Send SIGKILL to processes that didn't respond to SIGTERM

Every 60 seconds:
  - Reset circuit breaker counter
  - Log circuit breaker status
```

---

## Success Criteria

After system reboot, you will have succeeded if:

- ✅ Service is running without user login
- ✅ Service shows startup time ~60 seconds after boot
- ✅ Memory usage is 25-30MB
- ✅ CPU time is <1 minute (just started)
- ✅ Logs show clean startup message
- ✅ Circuit breaker shows state=Closed

---

## Common Questions

**Q: Does the daemon need me to be logged in?**
A: No! With linger enabled, it runs in the background even when logged out.

**Q: Will it restart if it crashes?**
A: Yes! Service file has `Restart=on-failure` configured.

**Q: Can I adjust sensitivity after reboot?**
A: Yes! Edit `~/.config/sysrespond/config.toml` and restart.

**Q: What if linger gets disabled?**
A: Daemon will stop on logout. Re-enable with `loginctl enable-linger`.

**Q: Will it survive multiple reboots?**
A: Yes! Linger is persistent across all reboots (stored in `/var/lib/systemd/linger/`).

---

## After Successful Verification

Once you've verified daemon auto-starts after reboot:

1. **Document the verification**: Save screenshot or note the timestamp
2. **Configure if needed**: Adjust thresholds in config.toml
3. **Monitor**: Check logs weekly with `journalctl --user -u sysrespond.service -n 50`
4. **Enjoy**: System is now protected from hung processes 24/7!

---

**Last Updated**: 2025-10-24
**Status**: Ready for verification on next reboot

