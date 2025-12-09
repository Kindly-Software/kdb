# Phase 2 Circuit Breaker Benchmarks - Quick Start Guide

**Status**: ✅ Benchmarks Ready
**Date**: 2025-10-17
**Framework**: B32 Compliant (Fair baselines, statistical rigor, realistic workloads)

---

## Quick Start

### Run All Phase 2 Benchmarks
```bash
cd /home/samuel/Primitives/clapi_core

# Run both benchmark suites (circuit breaker + provider routing)
cargo bench --bench circuit_breaker_metrics_bench --bench provider_circuit_bench
```

### Run Individual Benchmark Suites
```bash
# Circuit breaker operations (10 benchmarks)
cargo bench --bench circuit_breaker_metrics_bench

# Provider routing operations (12 benchmarks)
cargo bench --bench provider_circuit_bench
```

---

## Benchmark Coverage

### CircuitBreakerCapsule (10 benchmarks)
1. `allows_operation()` - Circuit check (<10ns)
2. `record_success()` - Success recording (<20ns)
3. `record_failure()` - Failure recording (<20ns)
4. `get_state()` - State snapshot (<30ns)
5. `open_circuit()` - Manual circuit open (<30ns)
6. Realistic pattern (90% success, 10% failure)
7. Concurrent check (4 threads)
8. Concurrent record (4 threads)
9. Concurrent record (8 threads)
10. Circuit trip detection (<5ns cached)

### RoutingCapsule128 (12 benchmarks)
1. `select_provider()` - Primary healthy (~80ns)
2. `select_provider()` - Failover (~90ns)
3. `select_provider()` - All unavailable (~100ns)
4. `update_state()` - Health update (~30ns)
5. `get_provider_id()` - ID lookup (~10ns)
6. `get_counters()` - Counter access (~10ns)
7. Realistic routing pattern (90/10 healthy/degraded)
8. Concurrent selection (4 threads)
9. Concurrent update (4 threads)
10. Concurrent mixed workload (8 threads, 80% reads/20% writes)
11. `circuit_generation()` - Generation counter (~15ns)
12. Scaling validation (1/2/4/8 threads)

---

## Expected Results

### Single-Threaded Performance
| Operation | Expected | Reality Check |
|-----------|----------|---------------|
| Circuit check | ~10ns | K2: Single atomic load |
| Record success | ~20ns | K2: CAS loop typical |
| Record failure | ~20ns | K2: CAS loop typical |
| Provider selection | ~80ns | K2: 2 atomic loads + state check |
| State update | ~30ns | K2: CAS loop + generation |

### Concurrent Performance (4 Threads)
| Operation | Expected | Reality Check |
|-----------|----------|---------------|
| Circuit check | ~40ns | K12: Lockfree read scaling |
| Record operations | ~50ns | K12: Moderate CAS contention |
| Provider selection | ~120ns | K12: Lockfree routing |
| State updates | ~60ns | K12: CAS contention on writes |

### Concurrent Performance (8 Threads)
| Operation | Expected | Reality Check |
|-----------|----------|---------------|
| Record operations | ~80ns | K12: Higher CAS contention |
| Mixed workload | ~100ns avg | K12: 80% reads, 20% writes |

---

## Viewing Results

### HTML Reports (Recommended)
```bash
# Run benchmarks
cargo bench --bench circuit_breaker_metrics_bench

# Open HTML report in browser
open target/criterion/report/index.html
```

### Terminal Output
Criterion prints detailed statistics to stdout:
```
circuit_breaker_check/circuit_breaker_capsule
                        time:   [9.8 ns 10.1 ns 10.4 ns]
                        change: [+9ns from baseline]

Benchmarking circuit_breaker_concurrent_check_4t/circuit_breaker_capsule_4t...
                        time:   [38.5 ns 41.2 ns 44.1 ns]
                        thrpt:  [97.3 Melem/s]
```

---

## Comparing Results

### Save Baseline
```bash
# Run benchmarks and save as baseline
cargo bench --bench circuit_breaker_metrics_bench -- --save-baseline phase2_v1
```

### Compare Against Baseline
```bash
# Later, compare new results against baseline
cargo bench --bench circuit_breaker_metrics_bench -- --baseline phase2_v1
```

### Example Comparison Output
```
circuit_breaker_check/circuit_breaker_capsule
                        time:   [10.1 ns 10.4 ns 10.7 ns]
                        change: [+0.3ns +2.9% +5.6%] (p = 0.03 < 0.05)
                        Performance has regressed.
```

---

## Troubleshooting

### Benchmark Compilation Errors
```bash
# Check benchmark compilation
cargo check --benches

# Common issues:
# - Missing capsule types: Check src/capsules/mod.rs exports
# - Import errors: Verify use statements in benchmarks
```

### Inconsistent Results
```bash
# Reduce system noise:
# 1. Close other applications
# 2. Disable CPU frequency scaling
# 3. Run multiple times to verify consistency

# Increase sample size for more stable results
cargo bench --bench circuit_breaker_metrics_bench -- --sample-size 2000
```

### Performance Below Expectations
```bash
# Factors affecting performance:
# - Thermal throttling (check CPU temperature)
# - Background processes (close unnecessary apps)
# - Debug mode (always use --release for benchmarks)

# Verify Criterion is using release mode
cargo bench --bench circuit_breaker_metrics_bench --verbose
```

---

## B32 Compliance Validation

### Fair Baselines (B1)
- ✅ **CircuitBreaker**: No protection (minimal overhead, not strawman mutex)
- ✅ **Routing**: Direct array lookup (realistic baseline)

### Statistical Rigor (B2)
- ✅ **95% confidence intervals**: Criterion default
- ✅ **1000+ samples**: Configured in benchmarks
- ✅ **Multiple runs**: Run 3× to verify consistency

### Realistic Workloads (B3)
- ✅ **90% success, 10% failure**: Production-like failure rates
- ✅ **Mixed read/write**: 80% reads, 20% writes (realistic)
- ✅ **Failover scenarios**: Primary unavailable → fallback routing

### Contention Scenarios (B4)
- ✅ **1 thread**: Uncontended baseline
- ✅ **4 threads**: Light/moderate contention
- ✅ **8 threads**: Heavy contention (lockfree stress test)

### Full Disclosure (B5)
- ✅ **Complete methodology**: See PHASE2_CIRCUIT_BREAKER_BENCHMARK_SUMMARY.md
- ✅ **Expected results**: All benchmarks documented with targets
- ✅ **Hardware constraints**: B32 K2, K12, K27 reality checks applied

---

## Integration with CI/CD

### GitHub Actions Example
```yaml
name: Phase 2 Benchmarks

on:
  push:
    branches: [main]
  pull_request:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run Phase 2 benchmarks
        run: |
          cd clapi_core
          cargo bench --bench circuit_breaker_metrics_bench --bench provider_circuit_bench
      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: target/criterion/
```

---

## Performance Regression Detection

### Automated Regression Checks
```bash
# Save baseline on main branch
git checkout main
cargo bench --bench circuit_breaker_metrics_bench -- --save-baseline main_baseline

# Later, on feature branch, compare against main
git checkout feature/my-optimization
cargo bench --bench circuit_breaker_metrics_bench -- --baseline main_baseline

# If performance regressed, Criterion will warn:
# "Performance has regressed."
```

### Acceptable Performance Variance
- **±10%**: Acceptable variance (noise, thermal, OS scheduling)
- **±20%**: Investigate potential regression
- **>20%**: Significant regression, requires investigation

---

## Files Reference

### Benchmark Files
- **`benches/circuit_breaker_metrics_bench.rs`** - CircuitBreakerCapsule benchmarks
- **`benches/provider_circuit_bench.rs`** - RoutingCapsule128 benchmarks

### Documentation
- **`PHASE2_CIRCUIT_BREAKER_BENCHMARK_SUMMARY.md`** - Executive summary (expected results, B32 compliance)
- **`RUN_PHASE2_BENCHMARKS.md`** - This quick start guide

### Output Directories
- **`target/criterion/`** - HTML reports and CSV data
- **`target/criterion/report/index.html`** - Main HTML report

---

## Next Steps

1. **Run Benchmarks**: Execute benchmarks on production hardware
2. **Validate Results**: Compare actual vs expected (within ±20%)
3. **Document Findings**: Update summary with actual measurements
4. **Save Baseline**: Establish baseline for future comparisons
5. **CI/CD Integration**: Add to automated performance tracking

---

## Support

For questions or issues:
- **Benchmark Methodology**: See B32_BENCHMARK_FRAMEWORK.md
- **Expected Results**: See PHASE2_CIRCUIT_BREAKER_BENCHMARK_SUMMARY.md
- **UCE33 Framework**: See UCE33_FRAMEWORK.md (tier selection, validation)
- **Hardware Reality**: See B32 K1-K50 reality checks

---

**Status**: ✅ Benchmarks Ready for Execution
**Framework**: B32 (Fair baselines, statistical rigor, realistic workloads)
**Expected**: 10-30ns circuit breaker overhead, 80ns health-aware routing (REALISTIC)
