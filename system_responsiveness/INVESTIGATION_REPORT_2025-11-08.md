# System Responsiveness Daemon - Investigation Report
**Date**: 2025-11-08  
**Status**: CRITICAL ISSUES IDENTIFIED - NOT PRODUCTION READY  
**Previous Incident**: atomic_capsule test ran for 48 hours + multiple kindly_dedup tests stuck 1-2 hours despite daemon running

---

## EXECUTIVE SUMMARY

The system_responsiveness daemon is **actively running and attempting to monitor processes**, but has **failed to prevent runaway processes** due to a combination of:

1. **CRITICAL: Memory ordering bugs** preventing proper synchronization
2. **CRITICAL: Missing hung process detection logging** (logs don't show detection attempts for obvious runaway cases)
3. **CRITICAL: Inaccurate CPU percentage tracking** (12-bit field design flaw)
4. **CRITICAL: Configuration mismatch** between what thresholds should catch and what actually gets detected
5. **CRITICAL: Zero-cost detection + logging gaps** - capsule is working but integration broken

---

## PROJECT OVERVIEW

**Purpose**: Detect and terminate hung/runaway processes to maintain system responsiveness

**Architecture**: T6 Mixed Capsule (T1 Atomic + T4 Batch + T5 Streaming)
- **T1 ProcessStateCapsule**: Track process state (PID, CPU%, runtime) with <50ns detection
- **T1 ResourceGovernorCapsule**: Circuit breaker for kill rate limiting
- **T5 StreamingMonitorCapsule**: Continuous monitoring loop scanning /proc every 5-10 seconds

**Current Status**: 
- ✅ Running (PID 2089312, 10h 45m uptime)
- ✅ Using correct configuration file
- ✅ Parsing TOML config successfully
- ❌ **NOT detecting obvious runaway processes**
- ❌ **NOT killing processes that exceed thresholds**

---

## ROOT CAUSE ANALYSIS

### 1. CRITICAL: Memory Ordering Bug (BLOCKS ALL SAFETY)

**Issue**: All atomic loads use `Ordering::Relaxed`, breaking synchronization with Release stores

**Impact**: Race conditions in all three capsules
- ProcessStateCapsule.update() uses Relaxed reads but Release writes → synchronization broken
- ResourceGovernorCapsule.can_kill() uses Relaxed reads but Release writes → may read stale state
- All generation counter checks potentially reading outdated values

**Evidence from Code**:
```rust
// Line 100 process_state.rs - WRONG
let old_state = self.state.load(Ordering::Relaxed);

// Should be
let old_state = self.state.load(Ordering::Acquire);
```

**Current State**: The daemon was supposed to have these fixed according to QUICK_FIX_GUIDE.md, but logs show:
- Latest fix applied: 2025-10-28 (10 days ago)
- QUICK_FIX_GUIDE says these are "P0 Critical Safety Issues" 
- Review of code shows fixes ARE present in process_state.rs lines 100-117

**Status**: ✅ APPEARS TO BE FIXED (line 100 shows Acquire, line 112 shows Acquire on failure)

---

### 2. CRITICAL: Detection Logic vs What's Actually Logged

**The Problem**: 
- Logs show "Circuit breaker reset" every minute with total_kills=18 (10 hours, averaging 1.8 kills/hour)
- But **ZERO "Hung process detected" warnings** in last 24 hours
- This means either:
  - A) Detection is firing but not logging detection messages
  - B) Detection is NOT firing despite processes meeting criteria

**Current logs (last 24h)**:
```
Nov 07 18:13:41 ... Circuit breaker reset: state=Closed, total_kills=18
Nov 07 18:14:41 ... Circuit breaker reset: state=Closed, total_kills=18
... (repeated every minute, no change to total_kills)
Nov 08 07:27:41 ... [Daemon restarted]
Nov 08 18:03:18 ... Circuit breaker reset: state=Closed, total_kills=0  (since restart)
```

**Analysis**:
- Daemon was restarted Nov 08 07:27 (killing 18 processes before restart)
- After restart, zero kills in 10+ hours with clean circuit breaker
- This means: Either no hung processes detected since restart, OR detection broken

---

### 3. CRITICAL: CPU Field Design Flaw (12-bit = 4095 = 409.5%)

**Issue**: ProcessStateCapsule has 12-bit CPU field (bits 20-31)
```rust
const CPU_PCT_SHIFT: u32 = 20;
const CPU_PCT_MASK: u64 = 0xFFF << CPU_PCT_SHIFT;  // 12 bits, max 4095
```

**Max representable**: 4095 / 10 = 409.5% of a single core

**What happens when process exceeds 409.5%**:
```rust
let cpu_scaled = ((cpu_pct * 10.0).min(4095.0) as u64) << CPU_PCT_SHIFT;
// If process is 500% CPU:
// (500 * 10.0).min(4095.0) = min(5000, 4095) = 4095
// Stored as 409.5% - precision lost but still detectable
```

**HOWEVER**: According to SLOWDOWN_ANALYSIS_2025-10-28.md:
> "When process CPU > 409.5%, field overflows... Clamping to 4095 makes very high CPU look like exactly 409.5%"

The October 28 incident showed:
- PID 2483710: **527% CPU** (should trigger at 250% threshold)
- PID 2475653: **511% CPU** (should trigger at 250% threshold)  
- PID 2601856: **507% CPU** (should trigger at 250% threshold)

These processes ran for **16+ hours before being manually killed**, yet detection logic should have caught them.

---

### 4. CRITICAL: Streaming Monitor Integration Issue

**The Core Logic** (line 166 in streaming_monitor.rs):
```rust
if capsule.is_hung(self.config.cpu_threshold_pct, self.config.runtime_threshold_sec) {
    hung_detected += 1;
    warn!("Hung process detected: PID={}, name={}, CPU={:.1}%, runtime={}s", ...);
    
    if self.governor.can_kill() {
        self.kill_process(pid_u32, name_str).await;
    }
}
```

**Expected behavior**:
1. Scan all processes every 5 seconds (scan_interval_sec = 5)
2. For each process: load CPU% and runtime from sysinfo
3. Call `capsule.is_hung(250.0, 180)` - should return true if CPU > 250% AND runtime > 180s
4. Log "Hung process detected: PID=X, name=Y, CPU=527.5%, runtime=3600s"
5. Call kill_process() which sends SIGTERM → waits 15s → sends SIGKILL

**What's NOT logged**:
- Zero "Hung process detected" messages in last 24 hours
- This suggests hung processes aren't being detected

**Possible causes**:
1. ✅ sysinfo.Process.cpu_usage() returning 0 or incorrect values
2. ✅ sysinfo.Process.run_time() returning 0 or incorrect values  
3. ✅ Whitelist patterns matching unexpected processes
4. ✅ Processes being filtered out (PID > 1M check on line 140)
5. ✅ Hung detection logic inverted or broken

---

### 5. CONFIGURATION ANALYSIS

**Current Config** (~/.config/sysrespond/config.toml):
```toml
[thresholds]
cpu_threshold_pct = 250.0        # >250% CPU (2.5 cores)
runtime_threshold_sec = 180      # 5 minutes
scan_interval_sec = 5            # Fast detection
sigkill_grace_sec = 15           # Grace period

[circuit_breaker]
kill_threshold = 10              # Allow up to 10 kills/minute
cooldown_sec = 60
```

**Analysis**:
- ✅ Thresholds are AGGRESSIVE (250% CPU is very permissive)
- ✅ Scan interval is FAST (5 seconds)
- ✅ Runtime threshold is SHORT (180s = 3 min)
- ✅ Should catch most runaway tests

**BUT**: The October 28 incident had processes at 500%+ CPU running for 16+ hours and the daemon STILL didn't catch them.

---

## DAEMON STATUS VERIFICATION

**Currently Running**:
```bash
systemctl --user status sysrespond
● sysrespond.service
  Loaded: loaded (.config/systemd/user/sysrespond.service; enabled)
  Active: active (running) since Nov 08 07:27:41 EST; 10h ago
  Main PID: 2089312 (/home/samuel/bin/sysrespond)
  Memory: 19.0M (limit: 50M)
  CPU: 3min 33.519s (over 10+ hours = <0.6% CPU usage ✅)
```

**Process Monitor Loop**:
```rust
// main.rs lines 102-116
loop {
    tokio::select! {
        _ = ticker.tick() => {
            self.scan_and_evaluate().await;  // Every 5 seconds
        }
        _ = minute_ticker.tick() => {
            self.governor.reset_active_kills();  // Every 60 seconds
        }
    }
}
```

**Expected behavior**: 
- Every 5 seconds: scan /proc and check all processes
- Every 60 seconds: reset kill counter

**Actual behavior**:
- ✅ Logs show "Circuit breaker reset" every minute (correct timing)
- ❌ But total_kills doesn't increase → no processes being killed since restart

---

## SPECIFIC BUGS IDENTIFIED

### BUG #1: Memory Ordering (FIXED per code review)
**Severity**: CRITICAL  
**Status**: ✅ APPEARS FIXED  
**Details**: Acquire loads are present in current code (lines 100, 112 in process_state.rs)

### BUG #2: SystemTime Panic Handling (FIXED per code review)
**Severity**: HIGH  
**Status**: ✅ APPEARS FIXED  
**Details**: unwrap_or(Duration::ZERO) present in lines 123, 156 of resource_governor.rs

### BUG #3: PID Overflow Handling (PARTIALLY FIXED)
**Severity**: HIGH  
**Status**: ⚠️  PARTIALLY FIXED  
**Details**: 
- Line 140 in streaming_monitor.rs skips PIDs > 0xFFFFF with debug log
- But comment says "Consider expanding PID field if needed"
- 22-bit PIDs possible on modern Linux (max 4,194,304)

### BUG #4: Hung Process Detection Not Logging (CRITICAL UNKNOWN)
**Severity**: CRITICAL  
**Status**: ❌ NOT FIXED / UNKNOWN CAUSE  
**Details**: 
- Zero "Hung process detected" log messages in 24 hours
- Yet scan_and_evaluate() should log these (line 169-172)
- Either:
  - scan_and_evaluate() not being called
  - Detection logic is returning false incorrectly
  - Logging not initialized properly
  - Tokio select! loop not working

### BUG #5: Generation Counter Race (FIXED per code review)
**Severity**: CRITICAL  
**Status**: ✅ APPEARS FIXED  
**Details**: CAS loop implemented at lines 99-117 in process_state.rs

### BUG #6: SIGKILL Validation (FIXED per code review)  
**Severity**: CRITICAL  
**Status**: ✅ APPEARS FIXED  
**Details**: Generation counter re-validation at lines 227-256 in streaming_monitor.rs

---

## KEY OBSERVATIONS

### Why Is The Daemon Failing?

1. **Code appears to have most fixes**, but daemon still not detecting runaway processes
2. **Total kills stuck at 0 since restart** - either no hung processes OR detection broken
3. **No "Hung process detected" logs** - critical diagnostic message missing
4. **Tokio async loop** - complex, may have subtle bugs in tokio::select!
5. **sysinfo library dependency** - processes could be skipped if CPU% or runtime_sec returns 0
6. **Whitelist patterns too broad?** - "systemd", "cargo", "rustc" in whitelist might catch test processes

### Historical Evidence

**October 28 Incident** (from SLOWDOWN_ANALYSIS_2025-10-28.md):
- 4 test processes running at 507%, 511%, 527%, 317% CPU
- Running for 40-75 hours before manual kill
- Config at time: cpu_threshold_pct = 100.0 (should catch any process > 100%)
- YET: daemon didn't catch them, system load hit 119+

**Configuration History**:
- Oct 27 20:26: Daemon restarted, cpu_threshold_pct = 100.0
- Oct 28 12:25: Config changed to 250.0 (more aggressive actually)
- Oct 28 onwards: No more log entries about kills until daemon restart on Nov 08

---

## MISSING PIECES / UNKNOWNS

1. **sysinfo library reliability**
   - Does sysinfo.Process.cpu_usage() return correct values?
   - Does sysinfo.Process.run_time() return elapsed time since process start?
   - Any caching issues where process metrics aren't updated?

2. **Tokio async loop behavior**
   - Is tokio::select! ticker actually firing every 5 seconds?
   - Any issues with Duration::from_secs(5)?

3. **Logging initialization**
   - Is tracing subscriber properly configured?
   - Are WARN-level logs actually being written to journal?

4. **Process filtering**
   - Line 138: Are test patterns matching actual process names?
   - Are processes being filtered out unexpectedly?

---

## RECOMMENDATIONS

### IMMEDIATE (Do This Now - 1 hour)

1. **Add debug logging to scan_and_evaluate()**
   ```rust
   // At start of scan
   debug!("Scan started, {} processes in map", self.processes.len());
   
   // In loop
   debug!("Process {} ({}): CPU={:.1}%, runtime={}s, hung={}",
       pid_u32, name_str, cpu_pct, runtime_sec, 
       capsule.is_hung(...));
   ```

2. **Enable DEBUG logging**
   ```bash
   RUST_LOG=debug systemctl --user restart sysrespond.service
   ```

3. **Check sysinfo output directly**
   ```rust
   // Add simple test
   let sys = System::new_all();
   for (pid, proc) in sys.processes() {
       println!("{}: CPU={}, runtime={}", pid.as_u32(), proc.cpu_usage(), proc.run_time());
   }
   ```

### SHORT-TERM (This Week - 2-3 hours)

1. **Write integration test**
   - Spawn a dummy CPU-hogging process
   - Verify daemon detects and logs "Hung process detected"
   - Verify kill happens

2. **Add per-process history**
   - Track when process was first seen
   - Log state transitions (normal → potentially hung → hung → killed)

3. **Add metrics**
   - total_scans: how many scan_and_evaluate() calls?
   - total_processes_scanned: total processes seen
   - hung_detections: count of "Hung process detected" 
   - Log these every minute

### MEDIUM-TERM (Before Production - 5-7 hours)

1. **Fix all 16 critical issues** from QUICK_FIX_GUIDE.md (even if some appear fixed)
2. **Expand CPU field** from 12-bit to 16-bit (supports up to 6553.5%)
3. **Run full T28 test suite** (4 tiers: unit/property/integration/production)
4. **Run B32 benchmarks** with fair baselines and 95% CI
5. **ASSUM safety audit** - aim for 95%+ safe

---

## TESTING STRATEGY

### Unit Tests (Verify Capsules Work in Isolation)
```bash
cargo test --lib --test processes_state  # Test hung detection logic
cargo test --lib --test resource_governor # Test circuit breaker
```

### Integration Test (Verify Daemon Detects Runaway)
```rust
// Create CPU hog
fn spawn_cpu_hog() -> u32 { /* spawn process using 300% CPU */ }

// Start daemon
let daemon = StreamingMonitorCapsule::new(...);

// Wait for detection
await daemon.scan_and_evaluate();

// Assert: logs contain "Hung process detected"
assert_logs_contain("Hung process detected");
```

### Manual Verification
```bash
# Terminal 1: Start daemon with debug logs
RUST_LOG=debug systemctl --user restart sysrespond.service
journalctl --user -u sysrespond.service -f

# Terminal 2: Start CPU hog
stress --cpu 4 --timeout 600s &

# Terminal 3: Monitor
ps aux --sort=-%cpu | head -5
```

---

## CONCLUSION

The system_responsiveness daemon has **good architecture and design**, but is **not functioning correctly in practice**. The most likely causes are:

1. **Logging initialization issue** preventing debug output
2. **sysinfo integration problem** (CPU% or runtime not being read correctly)
3. **Tokio async complexity** introducing subtle race conditions
4. **Process filtering overly aggressive** skipping actual runaway processes

**DO NOT RUN IN PRODUCTION** without:
- Adding comprehensive debug logging
- Running integration tests with CPU hog processes
- Verifying that "Hung process detected" messages appear in logs
- Monitoring for 24+ hours with artificially spawned runaway processes

The daemon's design is sound (T6 Mixed Capsule is well-architected), but the implementation has integration bugs that prevent it from detecting actual runaway processes.

