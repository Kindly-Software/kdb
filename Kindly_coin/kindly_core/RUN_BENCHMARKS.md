# Quick Start: Running Kindly Core Benchmarks

**B32 Framework Compliant Benchmarking Guide**

## Prerequisites

1. **Rust**: Nightly toolchain (for `portable_simd` feature)
2. **Hardware**: Intel Ultra 7 155H (or document your hardware)
3. **System**: Linux (for CPU governor control)

## Quick Commands

### Run All Benchmarks

```bash
# From kindly_core directory
cargo bench

# Or from workspace root
cargo bench --package kindly_core
```

### Run Specific Benchmark Suite

```bash
# Transaction capsule benchmarks
cargo bench --bench transaction_capsule

# Block capsule benchmarks
cargo bench --bench block_capsule

# Account state capsule benchmarks
cargo bench --bench account_state_capsule
```

### Extended Measurement (for p99 accuracy)

```bash
# 20-second measurement time (better tail latency data)
cargo bench --bench transaction_capsule -- --measurement-time 20

# 50-second for production validation
cargo bench --bench account_state_capsule -- --measurement-time 50
```

## Hardware Setup (B32 Compliance)

### 1. Set CPU to Performance Mode

```bash
# View current governor
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Set to performance (requires sudo)
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

### 2. Disable Turbo Boost (for consistency)

```bash
# Check turbo status (0 = enabled, 1 = disabled)
cat /sys/devices/system/cpu/intel_pstate/no_turbo

# Disable turbo for consistent results
echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo
```

### 3. Minimize Background Processes

```bash
# Close unnecessary applications
# Stop non-essential services
systemctl list-units --type=service --state=running

# Check system load
htop
```

### 4. Pin to P-cores Only (optional)

```bash
# Intel Ultra 7 155H: P-cores are 0-5
taskset -c 0-5 cargo bench
```

## Interpreting Results

### Criterion Output

```
transaction_validation/validate_committed
                        time:   [98.234 ns 100.456 ns 102.678 ns]
                        change: [-2.3% +0.5% +3.2%] (p = 0.65 > 0.05)
                        Performance has not changed.
Found 8 outliers among 1000 measurements (0.8%)
  5 (0.5%) high mild
  3 (0.3%) high severe
```

**Reading the Results:**

1. **time: [P50, median, P95]**
   - First number: 50th percentile (98.234 ns)
   - Middle: Median (100.456 ns) ← **Primary metric**
   - Last: 95th percentile (102.678 ns)

2. **change: [lower, estimate, upper]**
   - Confidence interval vs previous run
   - p-value: >0.05 means no significant change

3. **outliers**
   - <5% is good (0.8% is excellent)
   - High mild: 1.5x IQR above Q3
   - High severe: 3x IQR above Q3

### Performance Targets

| Operation | Target | Baseline | Expected |
|-----------|--------|----------|----------|
| Transaction validation | <500ns | 10-15ns atomic | 100-500ns |
| Block finality check | <100ns | 5-10ns atomic | 50-100ns |
| Account balance read | <50ns | 5-10ns atomic | 20-50ns |
| Account balance update | <100ns | 10-15ns CAS | 50-100ns |

**Success Criteria:**
- ✅ Median < target
- ✅ P95 < 1.5× target
- ✅ Outliers < 5%
- ✅ Reproducible (±5% across runs)

## Regression Detection

### Save Baseline

```bash
# Save current performance as baseline
cargo bench --package kindly_core -- --save-baseline main

# Save for specific feature
cargo bench --package kindly_core -- --save-baseline v1.0.0
```

### Compare Against Baseline

```bash
# Compare current vs baseline
cargo bench --package kindly_core -- --baseline main

# Will show regression/improvement percentages
```

### Example Output

```
transaction_validation/validate_committed
                        time:   [100.4 ns 102.5 ns 104.6 ns]
                        change: [+8.2% +10.5% +12.8%] (p = 0.00 < 0.05)
                        Performance has regressed.
```

**Action on Regression:**
- >5% regression: Investigate
- >10% regression: Block PR/commit
- >20% regression: Critical issue

## Advanced Usage

### Profile Specific Benchmark

```bash
# Run with profiler (Linux perf)
cargo bench --bench transaction_capsule -- --profile-time 10

# With flamegraph (requires flamegraph crate)
cargo flamegraph --bench transaction_capsule
```

### Export Results

```bash
# Criterion saves results to target/criterion/
ls target/criterion/transaction_validation/validate_committed/

# View HTML report
firefox target/criterion/report/index.html
```

### Custom Benchmark Parameters

```bash
# Increase sample size for high-precision measurements
cargo bench -- --sample-size 5000

# Reduce warmup time (faster iteration during dev)
cargo bench -- --warm-up-time 1

# Filter by benchmark name
cargo bench -- validate
```

## Expected Performance (Intel Ultra 7 155H)

Based on B32 hardware baselines:

### Transaction Capsule
- **baseline_atomic_read**: 10-15ns (hardware minimum)
- **validate_committed**: 50-100ns (target <500ns) ✅
- **validate_full**: 300-500ns (with checksum) ✅
- **publish_full**: 500ns-1μs (two-phase commit) ✅

### Block Capsule
- **baseline_atomic_read**: 10-15ns
- **finality_check**: 50-100ns (target <100ns) ✅
- **height_read_single**: 15-25ns ✅
- **consensus_pattern**: 40-80ns average ✅

### Account State
- **baseline_atomic_load**: 5-10ns
- **balance_fast**: 10-20ns (may race) ✅
- **balance_consistent**: 30-50ns (two-phase) ✅
- **credit_100ns**: 60-100ns (CAS retry) ✅
- **concurrent_updates/4_threads**: 80-150ns per op ✅

## Troubleshooting

### High Variance (>15%)

**Symptoms:** Large confidence intervals, many outliers

**Solutions:**
1. Set CPU to performance mode (see Hardware Setup)
2. Disable turbo boost for consistency
3. Close background applications
4. Increase sample size: `--sample-size 2000`

### Performance Regression

**Symptoms:** >5% slower than baseline

**Debug Steps:**
1. Run baseline again: `cargo bench -- --baseline main`
2. Check for code changes in hot path
3. Profile with `perf record`
4. Validate compiler flags (should be `--release`)

### Benchmark Hangs

**Symptoms:** Benchmark never completes

**Likely Causes:**
1. Deadlock (impossible - 100% lockfree)
2. Infinite retry loop (check circuit breaker)
3. System overload (check `htop`)

**Solutions:**
1. Kill and restart: `Ctrl+C`
2. Check system resources
3. Reduce thread count: benchmark with `--threads 1`

## Validation Checklist

Before reporting benchmark results:

- [ ] CPU set to performance governor
- [ ] Turbo boost disabled (for consistency)
- [ ] Background processes minimized
- [ ] At least 1000 samples per benchmark
- [ ] Outliers <5%
- [ ] Reproducible (run 3x, variance <10%)
- [ ] Hardware documented (CPU, RAM, OS)
- [ ] Compiler version: `rustc --version`

## Example Session

```bash
# 1. Setup hardware
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo

# 2. Save baseline
cargo bench --package kindly_core -- --save-baseline main

# 3. Make changes
# ... edit code ...

# 4. Compare
cargo bench --package kindly_core -- --baseline main

# 5. Check regression
# Look for "Performance has regressed" messages

# 6. View report
firefox target/criterion/report/index.html
```

## Quick Reference

| Command | Purpose |
|---------|---------|
| `cargo bench` | Run all benchmarks |
| `cargo bench -- --save-baseline NAME` | Save baseline |
| `cargo bench -- --baseline NAME` | Compare to baseline |
| `cargo bench -- --sample-size N` | Custom sample size |
| `cargo bench -- --measurement-time N` | Measurement duration |
| `cargo bench -- FILTER` | Run matching benchmarks |
| `taskset -c 0-5 cargo bench` | Pin to P-cores |

## Documentation

- **Full Report**: See `BENCHMARK_REPORT.md`
- **B32 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`
- **Criterion Guide**: https://bheisler.github.io/criterion.rs/book/

---

**Quick Start Complete!** Run `cargo bench` to validate performance.
