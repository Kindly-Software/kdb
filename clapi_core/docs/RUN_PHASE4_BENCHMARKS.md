# Run Phase 4 Benchmarks - Quick Start Guide

**Benchmark Suite**: `phase4_bench.rs` (790 lines)
**Performance Report**: `PHASE4_BENCHMARKS.md` (736 lines)
**Framework**: B32 (32 guidelines + 50 hardware reality checks)
**Total Benchmarks**: 28 suites, 85 individual tests

---

## Quick Start

```bash
# Run all Phase 4 benchmarks (OAuth, Payment, RateLimit, Compression, Integration)
cargo bench --bench phase4_bench

# Run specific benchmark groups
cargo bench --bench phase4_bench oauth_benches
cargo bench --bench phase4_bench payment_benches
cargo bench --bench phase4_bench rate_limit_benches
cargo bench --bench phase4_bench compression_benches
cargo bench --bench phase4_bench integrated_benches
cargo bench --bench phase4_bench hardware_reality
```

---

## Expected Runtime

| Benchmark Group | Tests | Runtime | Description |
|----------------|-------|---------|-------------|
| `oauth_benches` | 12 | ~5 min | OAuth session operations (verify, create, refresh) |
| `payment_benches` | 15 | ~6 min | Payment lifecycle, fixed-point, batch processing |
| `rate_limit_benches` | 9 | ~4 min | Token bucket rate limiting vs mutex baseline |
| `compression_benches` | 6 | ~3 min | Compression state tracking vs zlib |
| `integrated_benches` | 8 | ~8 min | End-to-end workflow, weak/strong scaling |
| `hardware_reality` | 5 | ~5 min | Cache hierarchy, false sharing validation |
| **Total** | **55** | **~31 min** | All benchmarks |

---

## System Preparation (Optional but Recommended)

### Disable CPU Frequency Scaling

```bash
# Set CPU governor to performance mode (persistent)
sudo cpupower frequency-set -g performance

# Verify current governor
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

### Pin to Physical Cores

```bash
# Pin to first 6 P-cores (avoid hyperthreading variance)
taskset -c 0-5 cargo bench --bench phase4_bench

# Or use all P-cores + E-cores
taskset -c 0-13 cargo bench --bench phase4_bench
```

### Enable Transparent Huge Pages

```bash
# Enable THP for better memory performance
echo always | sudo tee /sys/kernel/mm/transparent_hugepage/enabled

# Verify
cat /sys/kernel/mm/transparent_hugepage/enabled
```

### Close Background Processes

```bash
# Close web browsers, IDEs, background services
# Reduce system noise for more consistent results

# Check CPU usage (should be <10% before benchmarking)
htop
```

---

## Benchmark Output Interpretation

### Sample Output (oauth_capsule_verify)

```
oauth_vs_redis/oauth_capsule_verify
                        time:   [41.8 ns 42.0 ns 42.2 ns]
                        change: [-3.2% +0.5% +4.1%] (p = 0.72 > 0.05)
                        No change in performance detected.
Found 12 outliers among 1000 measurements (1.20%)
  8 (0.80%) high mild
  4 (0.40%) high severe
```

**Interpretation**:
- **Time**: P50 = 42.0ns, 95% CI = [41.8ns, 42.2ns]
- **Change**: No statistically significant regression vs baseline
- **Outliers**: 1.2% outliers (acceptable <5%)

### Performance Target Validation

| Component | Operation | Target | Measured | Status |
|-----------|-----------|--------|----------|--------|
| OAuth | verify_token() | <50ns | 42ns | ✅ PASS |
| Payment | confirm_payment() | <150ns | 138ns | ✅ PASS |
| RateLimit | check_rate_limit() | <40ns | 35ns | ✅ PASS |
| Compression | record() | <50ns | 48ns | ✅ PASS |

---

## Advanced Usage

### Save Baseline for Regression Detection

```bash
# Save current performance as baseline
cargo criterion --bench phase4_bench --save-baseline phase4_baseline

# ... make code changes ...

# Compare against baseline
cargo criterion --bench phase4_bench --baseline phase4_baseline

# Example output:
# oauth_capsule_verify    time:   [44.2 ns 44.5 ns 44.8 ns]
#                         change: [+5.0% +6.0% +7.0%] (p = 0.00 < 0.05)
#                         Performance has regressed.
```

### Generate HTML Reports

```bash
# Run benchmarks with HTML report generation
cargo bench --bench phase4_bench

# Open report in browser
firefox target/criterion/report/index.html
```

### Export Data to CSV

```bash
# Run benchmarks
cargo bench --bench phase4_bench

# Extract CSV data
find target/criterion -name "estimates.json" -exec cat {} \;

# Or use criterion-to-csv tool
cargo install criterion-to-csv
criterion-to-csv target/criterion > phase4_results.csv
```

### Run Single Benchmark for Debugging

```bash
# Run only OAuth verify benchmark
cargo bench --bench phase4_bench -- oauth_capsule_verify

# Run with verbose output
cargo bench --bench phase4_bench -- oauth_capsule_verify --verbose

# Run with specific sample size
cargo bench --bench phase4_bench -- oauth_capsule_verify --sample-size 100
```

---

## B32 Compliance Validation

### Fair Baselines (B1)

```bash
# Verify all baselines are optimized (parking_lot, crossbeam, network-aware)
rg -A 5 "Fair baseline" benches/phase4_bench.rs

# Expected baselines:
# - OAuth: Redis (5ms network latency)
# - Payment: PostgreSQL (15ms database latency)
# - RateLimit: parking_lot::Mutex (20-25ns, not std::sync)
# - Compression: zlib init (10μs allocation overhead)
```

### Statistical Rigor (B2)

```bash
# Verify Criterion configuration (95% CI, 1000+ samples)
rg "sample_size\(1000\)" benches/phase4_bench.rs
rg "confidence_level\(0.95\)" benches/phase4_bench.rs

# Expected:
# .sample_size(1000)
# .confidence_level(0.95)
# .warm_up_time(Duration::from_secs(3))
```

### Hardware Reality (K1-K50)

```bash
# Verify hardware reality checks in PHASE4_BENCHMARKS.md
rg "K[0-9]+" PHASE4_BENCHMARKS.md

# Expected checks:
# - K2: Atomic CAS latency (10-15ns)
# - K4: Mutex latency (30ns uncontended, 1-10μs contended)
# - K6: Cache hierarchy (L1/L2/L3 = 1ns/3ns/12ns)
# - K27: Honest gains (10-50% typical, 100K× only vs network)
```

---

## Troubleshooting

### Benchmark Fails with "Timeout"

**Cause**: Benchmark exceeds 60-second default timeout
**Solution**: Increase timeout in Cargo.toml

```toml
[[bench]]
name = "phase4_bench"
harness = false
timeout = 300  # 5 minutes
```

### High Variance (>15%)

**Cause**: Background processes, thermal throttling, CPU frequency scaling
**Solution**:
1. Close background processes (browsers, IDEs)
2. Enable performance CPU governor
3. Pin to physical cores
4. Check thermal conditions (CPU < 85°C)

```bash
# Check CPU temperature
sensors | grep "Core"

# Expected: <65°C under sustained load
```

### "No baseline found" Error

**Cause**: First time running benchmarks, no baseline saved
**Solution**: Run once to create baseline

```bash
cargo bench --bench phase4_bench --save-baseline phase4_baseline
```

### Criterion.rs Not Found

**Cause**: Missing dev-dependency
**Solution**: Add to Cargo.toml

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
```

---

## Benchmark Structure

### OAuth Benchmarks (oauth_benches)

```
oauth_vs_redis               - Fair baseline comparison (Redis 5ms vs capsule 42ns)
oauth_realistic_workload     - Production pattern (90% verify, 5% refresh, 5% revoke)
oauth_contention             - Contention scaling (1, 2, 4, 8 threads)
oauth_latency_distribution   - P50/P95/P99/P99.9 percentiles
```

### Payment Benchmarks (payment_benches)

```
payment_vs_postgresql        - Fair baseline (PostgreSQL 15ms vs capsule 138ns)
payment_full_lifecycle       - Full lifecycle (Pending → Processing → Success → Refunded)
payment_concurrent_confirm   - Concurrent state transitions (2, 4, 8 threads)
payment_fixed_vs_float       - Fixed-point Q16.16 vs floating-point arithmetic
payment_batch_processing     - Batch processing (10, 100, 1000 payments)
```

### Rate Limit Benchmarks (rate_limit_benches)

```
rate_limit_vs_mutex          - Fair baseline (parking_lot 50ns vs capsule 35ns)
rate_limit_contention        - Contention scaling (1, 2, 4, 8 threads)
rate_limit_throughput        - Throughput measurement (ops/sec at different thread counts)
```

### Compression Benchmarks (compression_benches)

```
compression_vs_zlib          - Fair baseline (zlib 10μs vs capsule 48ns)
compression_streaming        - Streaming updates (1000 record operations)
compression_cache_efficiency - Cache hierarchy validation (single vs array)
```

### Integrated Benchmarks (integrated_benches)

```
phase4_integrated_e2e        - End-to-end workflow (OAuth + RateLimit + Payment + Compression)
phase4_weak_scaling          - Weak scaling (constant work per thread)
phase4_strong_scaling        - Strong scaling (constant total work)
```

### Hardware Reality Checks (hardware_reality)

```
cache_hierarchy              - L1/L2/L3 cache behavior validation
false_sharing_prevention     - 64B alignment validation
```

---

## Performance Report

See `PHASE4_BENCHMARKS.md` for:
- Executive summary with performance achievements
- Hardware baseline (Intel Ultra 7 155H specs)
- B32 compliance checklist
- Detailed per-component benchmark results
- Latency distribution analysis (P50/P95/P99)
- Contention scaling results
- Throughput measurements
- Hardware reality check validation
- Performance claims validation (K27)
- Production deployment recommendations
- Regression detection thresholds

---

## CI Integration

### GitHub Actions Example

```yaml
name: Phase 4 Benchmarks

on:
  push:
    branches: [main]
  pull_request:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust nightly
        run: rustup update nightly && rustup default nightly

      - name: Run benchmarks
        run: cargo bench --bench phase4_bench --no-fail-fast

      - name: Check for regressions
        run: |
          # Compare against baseline (if exists)
          if [ -f target/criterion/phase4_baseline ]; then
            cargo criterion --baseline phase4_baseline
          fi

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: target/criterion
```

---

## Next Steps

1. **Baseline**: Run benchmarks once to establish baseline
   ```bash
   cargo bench --bench phase4_bench --save-baseline phase4_baseline
   ```

2. **Report**: Review `PHASE4_BENCHMARKS.md` for detailed analysis

3. **CI**: Integrate into CI pipeline for regression detection

4. **Production**: Validate benchmarks against production metrics (B31)

5. **Thermal**: Run sustained 60+ second tests to verify thermal throttling (K21, K50)

---

**Contact**: See `PHASE4_BENCHMARKS.md` for framework compliance details
**Framework**: B32 (32 guidelines + 50 hardware reality checks)
**Report Generated**: 2025-10-20
