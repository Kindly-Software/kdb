# Chaos Engineering for atomic_mcp_server

**Version**: 1.0.0
**Status**: Production Ready
**Framework**: Lockfree chaos injection for resilience validation

## Overview

Chaos engineering validates system resilience through controlled failure injection. The atomic_mcp_server chaos framework uses **100% lockfree chaos injectors** to simulate failures without introducing synchronization overhead.

## Architecture

### Lockfree Chaos Injectors

All chaos injectors use atomic operations for zero-overhead failure injection:

```rust
pub struct NetworkChaos {
    config: ChaosConfig,
    active: AtomicBool,         // Lockfree activation
    packets_dropped: AtomicU64, // Lockfree counter
    packets_delayed: AtomicU64, // Lockfree counter
}
```

**Key Features**:
- **Zero mutex**: All coordination via atomics
- **Cache-aligned**: False sharing prevention
- **Configurable**: Failure rate, duration, recovery timeout
- **Composable**: Multiple chaos types simultaneously

---

## Chaos Types

### 1. Network Chaos

**Purpose**: Simulate network failures (packet loss, delays, partitions)

**Failures**:
- **Packet Drop**: Probabilistic packet loss (configurable rate)
- **Packet Delay**: Random delays (50-500ms range)
- **Network Partition**: Complete network failure simulation

**Configuration**:
```rust
let config = ChaosConfig::new("network")
    .with_failure_rate(0.1)  // 10% failure rate
    .with_duration(Duration::from_secs(5));

let chaos = NetworkChaos::new(config);
chaos.start();

// In request handler
if chaos.should_drop_packet() {
    return Err("Network unavailable");
}

if let Some(delay) = chaos.should_delay_packet() {
    thread::sleep(delay);
}

chaos.stop();
```

**Statistics**:
```rust
let (dropped, delayed) = chaos.stats();
println!("Packets: {} dropped, {} delayed", dropped, delayed);
```

**Success Criteria**:
- System remains responsive during network failures
- Automatic recovery after chaos stops
- No cascading failures

---

### 2. Disk Chaos

**Purpose**: Simulate disk failures (ENOSPC, EIO, slow I/O)

**Failures**:
- **ENOSPC**: Disk full simulation (no space left on device)
- **EIO**: I/O error simulation (disk read/write failures)
- **Slow I/O**: Artificial I/O delays

**Configuration**:
```rust
let config = ChaosConfig::new("disk")
    .with_failure_rate(0.2);  // 20% failure rate

let chaos = DiskChaos::new(config);
chaos.start();

// In audit log write
if chaos.should_fail_with_enospc() {
    return Err("No space left on device");
}

if chaos.should_fail_with_eio() {
    return Err("I/O error");
}

chaos.stop();
```

**Statistics**:
```rust
let (enospc, eio) = chaos.stats();
println!("Disk errors: {} ENOSPC, {} EIO", enospc, eio);
```

**Success Criteria**:
- Graceful degradation when disk full
- Audit log continues in memory if disk unavailable
- No data corruption

---

### 3. CPU Chaos

**Purpose**: Simulate CPU exhaustion and throttling

**Failures**:
- **CPU Throttling**: Artificial busy-wait to exhaust CPU
- **CPU Quota**: Simulate cgroup CPU limits

**Configuration**:
```rust
let config = ChaosConfig::new("cpu")
    .with_failure_rate(0.5);  // 50% chance of throttling

let chaos = CpuChaos::new(config);
chaos.start();

// In request handler (apply throttling)
chaos.maybe_throttle();  // Busy-wait 10-100ms if triggered

chaos.stop();
```

**Statistics**:
```rust
let throttle_events = chaos.stats();
println!("CPU throttle events: {}", throttle_events);
```

**Success Criteria**:
- Latency increases but system stable
- No crashes under CPU starvation
- Throughput degrades gracefully

---

### 4. Memory Chaos

**Purpose**: Simulate OOM conditions and memory pressure

**Failures**:
- **OOM Simulation**: Allocation failure simulation (very rare, 0.01% of failure_rate)
- **Memory Pressure**: Simulate memory exhaustion

**Configuration**:
```rust
let config = ChaosConfig::new("memory")
    .with_failure_rate(1.0);  // 100% to trigger OOM (actual rate 0.01 × 1.0 = 1%)

let chaos = MemoryChaos::new(config);
chaos.start();

// In memory allocation
if chaos.should_fail_allocation() {
    return Err("Out of memory");
}

chaos.stop();
```

**Statistics**:
```rust
let oom_simulations = chaos.stats();
println!("OOM simulations: {}", oom_simulations);
```

**Success Criteria**:
- No panic on allocation failure
- Graceful error handling
- Memory usage doesn't grow unbounded

---

### 5. Clock Chaos

**Purpose**: Simulate clock skew and time going backwards

**Failures**:
- **Clock Skew**: Time goes backwards (100-5000ms)
- **Time Jump**: Simulate NTP corrections

**Configuration**:
```rust
let config = ChaosConfig::new("clock")
    .with_failure_rate(0.5);

let chaos = ClockChaos::new(config);
chaos.start();

// Use chaos-aware time
let now = chaos.now_with_chaos();  // May go backwards

// Saturating arithmetic prevents underflow
let duration = now.saturating_sub(prev_time);

chaos.stop();
```

**Statistics**:
```rust
let total_skew_ns = chaos.total_skew_ns();
println!("Total clock skew: {} ns", total_skew_ns);
```

**Success Criteria**:
- No underflow panics
- Saturating arithmetic handles backwards time
- Timestamps remain monotonic in critical paths

---

## Chaos Coordinator

**Purpose**: Manage multiple chaos injectors simultaneously

**Usage**:
```rust
use atomic_mcp_server::chaos::ChaosCoordinator;

let coordinator = ChaosCoordinator::new();

// Start all chaos injectors
coordinator.start_all();

// Run production workload under chaos
run_production_load();

// Stop all chaos injectors
coordinator.stop_all();

// Print statistics
coordinator.print_stats();
```

**Output**:
```
Network: 127 packets dropped, 89 delayed
Disk: 42 ENOSPC, 23 EIO
CPU: 156 throttle events
Memory: 3 OOM simulations
Clock: 2,341,892 ns total skew
```

---

## Chaos Tests (Q24)

### Test 1: Network Partition

**Scenario**: Simulate network failure during request processing

**Steps**:
1. Start network chaos (50% packet drop rate)
2. Send 1000 requests
3. Verify: Timeout, retry, recovery

**Success Criteria**:
- Requests timeout gracefully (not hang)
- System recovers after chaos stops
- No cascading failures

**Run**:
```bash
cargo test --test chaos network_partition_test
```

---

### Test 2: Disk Full (ENOSPC)

**Scenario**: Simulate disk full during audit log write

**Steps**:
1. Start disk chaos (20% ENOSPC rate)
2. Write 1000 audit events
3. Verify: Graceful degradation, error logged

**Success Criteria**:
- Audit log degrades to memory-only mode
- No data loss for in-memory events
- System remains functional

**Run**:
```bash
cargo test --test chaos disk_full_test
```

---

### Test 3: OOM Simulation

**Scenario**: Simulate allocation failure

**Steps**:
1. Start memory chaos (100% failure rate → 1% actual)
2. Allocate memory under chaos
3. Verify: Error handling, no panic

**Success Criteria**:
- Allocation failures handled gracefully
- No panic on OOM
- System remains stable

**Run**:
```bash
cargo test --test chaos oom_test
```

---

### Test 4: Clock Skew

**Scenario**: Simulate time going backwards

**Steps**:
1. Start clock chaos (50% skew rate)
2. Measure timestamps 1000 times
3. Verify: Saturating arithmetic prevents underflow

**Success Criteria**:
- No panic on time underflow
- Saturating arithmetic used
- Timestamps handled correctly

**Run**:
```bash
cargo test --test chaos clock_skew_test
```

---

### Test 5: Signal Handling (SIGTERM)

**Scenario**: Send SIGTERM during request processing

**Steps**:
1. Start request processing
2. Send SIGTERM signal
3. Verify: Graceful shutdown, requests complete

**Success Criteria**:
- In-flight requests complete
- Graceful shutdown (no abrupt termination)
- Resources cleaned up

**Run**:
```bash
cargo test --test chaos signal_interruption_test
```

---

### Test 6: CPU Throttle

**Scenario**: Limit CPU to 25% during stress test

**Steps**:
1. Start CPU chaos (50% throttle rate)
2. Run stress test (1000 requests)
3. Verify: Higher latency but stable

**Success Criteria**:
- Latency increases proportionally
- No crashes
- Throughput degrades gracefully

**Run**:
```bash
cargo test --test chaos cpu_throttle_test
```

---

### Test 7: File Descriptor Exhaustion

**Scenario**: Exhaust FD limit (ulimit -n)

**Steps**:
1. Open connections until FD limit reached
2. Attempt one more connection
3. Verify: Connection rejected gracefully

**Success Criteria**:
- New connections rejected with EMFILE
- Existing connections remain functional
- No crash

**Run**:
```bash
cargo test --test chaos fd_exhaustion_test
```

---

### Test 8: DNS Timeout

**Scenario**: Simulate DNS resolution timeout

**Steps**:
1. Start network chaos (packet delay)
2. Attempt DNS resolution
3. Verify: Timeout, retry, recovery

**Success Criteria**:
- DNS timeout handled
- Retry mechanism works
- System recovers after chaos

**Run**:
```bash
cargo test --test chaos dns_timeout_test
```

---

### Test 9: Concurrent Component Failures

**Scenario**: Fail multiple components simultaneously

**Steps**:
1. Start all chaos injectors (network + disk + CPU + memory + clock)
2. Run production workload
3. Verify: Partial degradation, no cascade

**Success Criteria**:
- System remains partially functional
- No cascading failures
- Graceful degradation

**Run**:
```bash
cargo test --test chaos --all-features
```

---

## Chaos in Production

### Controlled Chaos Injection

**DO NOT** run chaos tests in production. Instead:

1. **Staging Environment**: Run chaos tests in staging before production deployment
2. **Canary Deployments**: Deploy to small subset of production traffic first
3. **Circuit Breakers**: Use circuit breaker pattern to isolate failures
4. **Feature Flags**: Gate chaos injection behind feature flags

### Monitoring During Chaos

**Metrics to track**:
- Request latency (P50/P95/P99)
- Error rate (4xx/5xx responses)
- Resource usage (CPU/memory/FDs)
- Circuit breaker state transitions

**Alerting**:
- Alert on high error rate (>10%)
- Alert on latency degradation (>2× baseline)
- Alert on resource exhaustion

### Recovery Procedures

**After chaos injection**:
1. **Stop chaos injectors**: `coordinator.stop_all()`
2. **Verify recovery**: Check metrics return to baseline
3. **Inspect logs**: Review error logs for unexpected failures
4. **Validate data integrity**: Ensure no corruption

---

## Framework Compliance

### Chaos (Computational Capsule Architecture)
- **100% Lockfree**: All chaos injectors use atomics (zero mutex)
- **Cache-Aligned**: Prevent false sharing
- **Generation Counters**: Safe concurrent updates

### ASSUM (Safety Verification)
- **#ASSUME_CHAOS_LOCKFREE**: All chaos injection lockfree (verified: grep 0 mutex)
- **#ASSUME_PROBABILISTIC_FAILURE**: fastrand RNG deterministic (seed-based reproducibility)
- **#ASSUME_RECOVERY**: System recovers after chaos stops (verified in tests)

### T28 (Testing Strategy)
- **Q24**: Chaos engineering tests (9 scenarios)
- **Property Tests**: Chaos + property-based testing combination
- **Integration Tests**: Chaos + real-world workflows

---

## Appendix: Chaos Configuration Reference

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `name` | String | (required) | Chaos experiment name |
| `failure_rate` | f64 | 0.1 (10%) | Probability of failure |
| `failure_duration` | Duration | 5s | How long chaos lasts |
| `recovery_timeout` | Duration | 30s | Max time to recover |

### Chaos Injector API

```rust
pub trait ChaosInjector {
    fn start(&self);
    fn stop(&self);
    fn is_active(&self) -> bool;
    fn stats(&self) -> ChaosStats;
}
```

### Fastrand RNG (Deterministic)

Chaos uses `fastrand` crate for deterministic failure injection:
- **Seed-based**: Reproducible chaos scenarios
- **Fast**: <10ns per random number
- **Thread-safe**: Each thread has own RNG state

**Reproduce chaos scenario**:
```rust
fastrand::seed(42);  // Set seed
let chaos = NetworkChaos::new(config);
chaos.start();
// ... same sequence of failures every run
```

---

## Next Steps

1. **Run chaos tests**: `cargo test --test chaos --all-features`
2. **Integrate into CI/CD**: Add to nightly builds
3. **Schedule chaos exercises**: Run weekly in staging
4. **Document failures**: Capture failure modes for runbooks

**Status**: ✅ Chaos Framework Production Ready (9 scenarios, 100% lockfree)
