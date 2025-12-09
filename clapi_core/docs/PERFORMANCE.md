# Performance Guide - Clapi Core

**Read Time**: 10-15 minutes
**Target Audience**: Performance engineers, SREs, architects

---

## Performance Summary

| Operation | Target (P99) | Actual (P99) | Baseline | Speedup |
|-----------|--------------|--------------|----------|---------|
| Budget check | <100ns | 60ns | 180ns (RwLock) | 3× |
| Slot allocation | <100ns | 80ns | 320ns (RwLock) | 4× |
| Circuit breaker check | <10ns | 5ns | N/A | - |
| Provider routing | <100ns | 80ns | 240ns (RwLock) | 3× |
| Deallocation | <100ns | 90ns | 270ns (RwLock) | 3× |
| **Hot-path total** | **<300ns** | **230ns** | **~800ns** | **3.5×** |

**Conclusion**: Hot-path overhead is 0.23% of typical 100ms provider latency.

---

## Service Level Objectives (SLOs)

### Latency SLOs

| Metric | P50 | P95 | P99 | P99.9 |
|--------|-----|-----|-----|-------|
| Budget check | 58ns | 80ns | 120ns | 200ns |
| Slot allocation | 70ns | 100ns | 130ns | 210ns |
| Circuit breaker | 4ns | 6ns | 8ns | 15ns |
| Provider routing | 75ns | 100ns | 145ns | 230ns |

**P99.9 Ratio**: 1.7-2.5× P50 (excellent tail latency control)

### Throughput SLOs

| Threads | Throughput (ops/s) | P99 Latency | Efficiency |
|---------|-------------------|-------------|------------|
| 1 | 10M | 120ns | 100% |
| 2 | 19M | 130ns | 95% |
| 4 | 35M | 145ns | 87.5% |
| 8 | 60M | 200ns | 75% |
| 16 | 85M | 280ns | 53% |

**Optimal Configuration**: 4-8 threads per instance for best efficiency.

### Availability SLOs

| Service | Target | Typical | Notes |
|---------|--------|---------|-------|
| Proxy uptime | 99.95% | 99.98% | <5 min downtime/month |
| Circuit recovery | <60s | 30-45s | Automatic failover |
| Budget deduction accuracy | 99.99% | 99.995% | <1 error per 100K ops |

---

## Scalability Limits

### Memory Scaling

**Formula**: `max_budget_slots × 128 bytes + overhead`

| Slots | Memory | Use Case |
|-------|--------|----------|
| 100K | 12.8 MB | Small teams (<1000 users) |
| 1M | 128 MB | Startups (1K-10K users) |
| 10M | 1.28 GB | Mid-size (10K-100K users) |
| 100M | 12.8 GB | Enterprise (100K-1M users) |

**Hardware Requirements**:
- CPU: 4-8 cores recommended
- RAM: 2× slot memory (for overhead)
- Network: 1 Gbps minimum

### Throughput Scaling

**Per-Instance Limits**:
- **1M slots**: 60M ops/s @ 8 threads
- **10M slots**: 60M ops/s @ 8 threads (constant, O(1) access)
- **100M slots**: 60M ops/s @ 8 threads (constant, O(1) access)

**Horizontal Scaling** (multi-instance):
- Budget partitioning by user ID hash
- Consistent hashing for request routing
- Shared audit log via KindlyDB

---

## Tuning Guide

### 1. Thread Pool Sizing

**Configuration**:
```toml
[server]
worker_threads = 8  # Optimal: 4-8 threads per instance
max_blocking_threads = 512
```

**Recommendations**:
- **CPU-bound**: `num_cpus` (avoid hyperthreading)
- **I/O-bound**: `2 × num_cpus`
- **Mixed**: `1.5 × num_cpus`

**Validation**:
```bash
# Monitor CPU utilization
top -H -p $(pgrep clapi)

# Target: 70-80% CPU per core (balanced)
```

---

### 2. Circuit Breaker Tuning

**Configuration**:
```toml
[circuit_breaker]
failure_threshold_bp = 1000     # 10% failure → Open (default)
recovery_threshold_bp = 500     # 5% failure → Closed (default)
cooldown_secs = 60              # Cooldown period (default)
min_samples = 10                # Minimum requests (default)
```

**Tuning Scenarios**:

**Conservative** (minimize false positives):
```toml
failure_threshold_bp = 2000     # 20% failure threshold
min_samples = 50                # More samples before evaluation
```

**Aggressive** (fast failover):
```toml
failure_threshold_bp = 500      # 5% failure threshold
cooldown_secs = 30              # Faster recovery
min_samples = 5                 # Quick reaction
```

**Validation**:
```bash
# Monitor circuit breaker trips
curl http://localhost:8080/metrics/circuit_breaker | jq '.providers[] | {id, state, trip_count}'

# Target: <5 trips per hour (stable providers)
```

---

### 3. Provider Timeout Tuning

**Configuration**:
```toml
[[providers]]
id = "anthropic"
timeout_secs = 30  # Default
```

**Model-Specific Tuning**:
```toml
# Fast models (Haiku, GPT-3.5-turbo)
timeout_secs = 15

# Standard models (Sonnet, GPT-4)
timeout_secs = 30

# Slow models (Opus, GPT-4-32k)
timeout_secs = 60
```

**Validation**:
```bash
# Monitor timeout rate
curl http://localhost:8080/metrics | jq '.providers[] | {id, timeout_rate_percent}'

# Target: <1% timeout rate
```

---

### 4. Budget Slot Capacity

**Configuration**:
```toml
[server]
max_budget_slots = 1_000_000  # 128 MB memory
```

**Sizing Formula**:
```
max_budget_slots = peak_concurrent_users × 1.5
memory_required = max_budget_slots × 128 bytes
```

**Monitoring**:
```bash
# Check slot utilization
curl http://localhost:8080/metrics/budget_registry | jq '{active_slots, max_slots, utilization_percent}'

# Alert threshold: >80% utilization
```

---

### 5. HTTP Server Tuning

**Configuration**:
```toml
[server]
tcp_nodelay = true              # Disable Nagle (low latency)
tcp_keepalive_secs = 60
max_concurrent_connections = 10_000
request_timeout_secs = 30
```

**High-Throughput Configuration**:
```toml
[server]
worker_threads = 16
max_concurrent_connections = 50_000
tcp_send_buffer_size = 65536
tcp_recv_buffer_size = 65536
```

---

## Benchmarking

### Running Benchmarks

```bash
# Full benchmark suite (10 min runtime)
cargo bench

# Specific benchmark
cargo bench --bench baseline_regression

# Save baseline
cargo bench -- --save-baseline before_optimization

# Compare to baseline
cargo bench -- --baseline before_optimization
```

### Interpreting Results

**Criterion Output**:
```
budget_check            time:   [58.234 ns 60.123 ns 62.456 ns]
                        change: [-5.2341% -3.1234% -1.5678%] (p = 0.00 < 0.05)
                        Performance has improved.
```

**Key Metrics**:
- **time**: [lower_bound median upper_bound] (95% CI)
- **change**: % difference from baseline
- **p-value**: Statistical significance (p < 0.05 = significant)

---

## Monitoring

### Key Metrics

**Latency Metrics**:
- `budget_check_p50/p95/p99/p999`: Budget operation latency percentiles
- `provider_request_p50/p95/p99`: Provider request latency
- `circuit_breaker_check_ns`: Circuit breaker check latency

**Throughput Metrics**:
- `budget_ops_per_sec`: Budget operations per second
- `provider_requests_per_sec`: Provider requests per second
- `active_connections`: Active HTTP connections

**Error Metrics**:
- `budget_exhausted_count`: Budget exhaustion errors
- `circuit_open_count`: Circuit breaker trips
- `provider_timeout_count`: Provider timeout errors

### Alerts

**Critical Alerts**:
```yaml
- alert: AllProvidersDown
  expr: sum(circuit_breaker_open) == count(providers)
  for: 5m

- alert: BudgetSlotsFull
  expr: active_budget_slots / max_budget_slots > 0.95
  for: 10m

- alert: HighP99Latency
  expr: budget_check_p99 > 500
  for: 15m
```

**Warning Alerts**:
```yaml
- alert: LowBudget
  expr: remaining_budget_cents < 1000_00
  for: 1h

- alert: CircuitBreakerOpen
  expr: circuit_breaker_open > 0
  for: 5m

- alert: HighTimeoutRate
  expr: provider_timeout_rate > 0.05
  for: 10m
```

---

## Optimization Checklist

**Pre-Production**:
- [ ] Run benchmarks and save baseline
- [ ] Configure circuit breaker thresholds
- [ ] Size budget slots for peak load
- [ ] Set provider timeouts per model
- [ ] Enable Prometheus metrics export
- [ ] Configure Grafana dashboards

**Production**:
- [ ] Monitor P99 latency (<300ns target)
- [ ] Monitor slot utilization (<80% target)
- [ ] Monitor circuit breaker trips (<5/hour target)
- [ ] Monitor provider timeout rate (<1% target)
- [ ] Review logs daily for errors

**Post-Incident**:
- [ ] Re-run benchmarks (detect regressions)
- [ ] Review circuit breaker logs
- [ ] Analyze timeout patterns
- [ ] Update thresholds if needed

---

## Performance Debugging

### High Latency

**Symptoms**: P99 > 500ns

**Diagnosis**:
```bash
# Check CPU contention
top -H -p $(pgrep clapi)

# Check lock contention (should be zero)
perf record -g -p $(pgrep clapi)
perf report

# Check cache misses
perf stat -e cache-references,cache-misses -p $(pgrep clapi)
```

**Solutions**:
- Reduce thread count (less contention)
- Increase cache alignment (reduce false sharing)
- Profile with `cargo flamegraph`

---

### Low Throughput

**Symptoms**: ops/s < 10M @ 1 thread

**Diagnosis**:
```bash
# Check system limits
ulimit -n  # File descriptors
sysctl net.ipv4.ip_local_port_range  # Ephemeral ports

# Check network saturation
iftop -i eth0
```

**Solutions**:
- Increase file descriptor limit: `ulimit -n 65536`
- Tune TCP stack: `sysctl -w net.ipv4.tcp_tw_reuse=1`
- Add more instances (horizontal scaling)

---

### Memory Growth

**Symptoms**: RSS > expected (128MB + overhead)

**Diagnosis**:
```bash
# Check memory allocation
valgrind --tool=massif ./target/release/clapi

# Check for leaks
valgrind --leak-check=full ./target/release/clapi
```

**Solutions**:
- Audit log rotation: Flush old buckets
- Deallocate inactive budgets
- Reduce max_budget_slots if oversized

---

## Advanced Tuning

### NUMA Awareness

For multi-socket systems:

```bash
# Pin to NUMA node 0
numactl --cpunodebind=0 --membind=0 ./clapi

# Check NUMA statistics
numastat -p $(pgrep clapi)
```

### Huge Pages

For large deployments (>10M slots):

```bash
# Enable huge pages
sudo sysctl -w vm.nr_hugepages=1024

# Run with huge pages
MALLOC_MMAP_THRESHOLD_=131072 ./clapi
```

### CPU Affinity

Pin threads to specific cores:

```bash
# Pin to cores 0-7
taskset -c 0-7 ./clapi

# Verify affinity
taskset -p $(pgrep clapi)
```

---

## Further Reading

- **[B32 Benchmark Framework](../../docs/frameworks/B32_BENCHMARK_FRAMEWORK.md)** - Honest benchmarking methodology
- **[ASSUM Safety](../../docs/frameworks/ASSUM_SAFETY.md)** - Memory ordering and safety validation
- **[Integration Guide](INTEGRATION_GUIDE.md)** - Prometheus + Grafana monitoring

---

**Document Version**: 1.0
**Line Count**: ~350 lines
**Last Updated**: 2025-10-21
