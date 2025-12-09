# Run CapsuleHash64 Benchmarks - Quick Start Guide

**Benchmark Suite**: `benches/capsule_hash64_bench.rs`
**Status**: ✅ Ready to run (14 benchmarks, 619 lines, B32-compliant)
**Date**: 2025-10-17

---

## Quick Start (Scalar Benchmarks - Stable Rust)

```bash
# Run all scalar benchmarks (no SIMD, works on stable Rust)
cargo bench --bench capsule_hash64_bench

# View HTML report
xdg-open target/criterion/index.html  # Linux
open target/criterion/index.html      # macOS
```

**Expected Time**: ~5 minutes for 12 scalar benchmarks (1000 samples each)

---

## SIMD Benchmarks (Nightly Rust Required)

```bash
# Switch to nightly (if not already)
rustup default nightly

# Run with SIMD feature enabled
cargo +nightly bench --bench capsule_hash64_bench --features simd

# View HTML report
xdg-open target/criterion/index.html
```

**Expected Time**: ~7 minutes for all 14 benchmarks (includes 2 SIMD-specific)

---

## Run Specific Benchmark Groups

### Scalar Benchmarks Only
```bash
cargo bench --bench capsule_hash64_bench -- scalar
```

### SIMD Benchmarks Only (nightly + simd feature)
```bash
cargo +nightly bench --bench capsule_hash64_bench --features simd -- simd
```

### Concurrent Benchmarks Only
```bash
cargo bench --bench capsule_hash64_bench -- concurrent
```

### Production Capsule Benchmarks
```bash
cargo bench --bench capsule_hash64_bench -- production
```

### Variable Field Count Analysis
```bash
cargo bench --bench capsule_hash64_bench -- variable
```

---

## Run Individual Benchmarks

```bash
# Scalar hash (4 fields)
cargo bench --bench capsule_hash64_bench -- bench_hash_scalar_4fields

# SIMD hash (4 fields) - requires nightly + simd feature
cargo +nightly bench --bench capsule_hash64_bench --features simd -- bench_hash_simd_4fields

# Incremental update
cargo bench --bench capsule_hash64_bench -- bench_hash_incremental_update

# Atomic operations
cargo bench --bench capsule_hash64_bench -- bench_hash_atomic

# Concurrent (4 threads)
cargo bench --bench capsule_hash64_bench -- concurrent_4t
```

---

## Benchmark Output Interpretation

### Example Output (Scalar)
```
capsule_hash64_bench/bench_hash_scalar_4fields/capsule_hash64_scalar
                        time:   [4.2 ns 4.3 ns 4.5 ns]
                        change: [-2.1% -0.8% +0.6%] (p = 0.28 > 0.05)
                        No change in performance detected.
Found 12 outliers among 1000 measurements (1.20%)
```

**Interpretation**:
- **Median time**: 4.3ns (P50)
- **95% CI**: [4.2ns, 4.5ns] (confidence interval)
- **Change**: -0.8% (negligible, p > 0.05)
- **Outliers**: 1.2% (acceptable, <5%)

### Example Output (SIMD vs Baseline)
```
capsule_hash64_bench/bench_hash_simd_4fields/capsule_hash64_simd
                        time:   [1.9 ns 2.0 ns 2.1 ns]

capsule_hash64_bench/bench_hash_simd_4fields/baseline_scalar
                        time:   [4.2 ns 4.3 ns 4.5 ns]

Speedup: 2.15× (SIMD vs scalar)
```

**Interpretation**:
- **SIMD**: 2.0ns (P50)
- **Baseline**: 4.3ns (P50)
- **Speedup**: 2.15× ✅ **Reality Check (K9)**: AVX2 2-2.5× typical

---

## Expected Results

### Scalar Performance (Stable Rust)

| Benchmark | Expected | Reality Check |
|-----------|----------|---------------|
| Hash (4 fields) | ~4-5ns | K2: Tight loop overhead |
| Incremental update | <1ns | K2: Single XOR operation |
| Atomic store | <5ns | K2: Relaxed AtomicU64 store |
| Atomic load | <5ns | K2: Relaxed AtomicU64 load |

### SIMD Performance (Nightly Rust + `simd` feature)

| Benchmark | Expected | Speedup | Reality Check |
|-----------|----------|---------|---------------|
| SIMD (4 fields) | ~2-3ns | 2-2.5× | K9: AVX2 typical |
| SIMD (8 fields) | ~3-4ns | 2.5-3× | K9: AVX-512 typical |

### Concurrent Performance (4 threads)

| Benchmark | Expected | Speedup | Reality Check |
|-----------|----------|---------|---------------|
| Compute (4T) | ~4× | 3.5-4× | K12: Zero contention |
| Store (4T) | ~4× | 3.5-4× | K12: Relaxed ordering |
| Verify (4T) | ~4× | 3.5-4× | K12: Read-heavy |

---

## Validation Checklist

After running benchmarks, verify:

✅ **Scalar Performance**:
- [ ] Hash (4 fields): 4-5ns range ✅
- [ ] Incremental update: <1ns ✅
- [ ] Atomic store: <5ns ✅
- [ ] Atomic load: <5ns ✅

✅ **SIMD Performance** (nightly only):
- [ ] SIMD (4 fields): 2-3ns range ✅
- [ ] SIMD (8 fields): 3-4ns range ✅
- [ ] Speedup: 2-4× vs baseline ✅

✅ **Concurrent Scaling**:
- [ ] 4-thread compute: ~4× speedup ✅
- [ ] 4-thread store: ~4× speedup ✅
- [ ] 4-thread verify: ~4× speedup ✅

✅ **Reality Checks**:
- [ ] SIMD speedup NOT >10× (K27: suspicious) ✅
- [ ] Concurrent scaling linear up to 4T (K12) ✅
- [ ] All results within 95% CI (statistical rigor) ✅

---

## Troubleshooting

### Issue: "error: package `clapi_core` cannot be built because it requires rustc 1.83 or newer"

**Solution**: Update Rust
```bash
rustup update
cargo --version  # Verify ≥1.83
```

### Issue: "error: target `capsule_hash64_bench` not found"

**Solution**: Ensure Cargo.toml has benchmark registered
```bash
grep -A 2 "capsule_hash64_bench" Cargo.toml
# Should show:
# [[bench]]
# name = "capsule_hash64_bench"
# harness = false
```

### Issue: "feature `simd` is required but not enabled"

**Solution**: Use `--features simd` flag
```bash
cargo +nightly bench --bench capsule_hash64_bench --features simd
```

### Issue: "error: benches require `criterion` dependency"

**Solution**: Ensure Cargo.toml has `[dev-dependencies]` section with criterion
```bash
grep -A 2 "criterion" Cargo.toml
# Should show:
# criterion = { version = "0.5", features = ["html_reports"] }
```

### Issue: Benchmarks take too long (>10 minutes)

**Cause**: Concurrent benchmarks spawn threads (overhead)
**Solution**: Run scalar benchmarks only
```bash
cargo bench --bench capsule_hash64_bench -- scalar
```

---

## Hardware Considerations

### CPU Features

**Recommended**:
- **CPU**: Intel Core i7/i9 or AMD Ryzen 5/7/9
- **Features**: AVX2 (for SIMD benchmarks)
- **Cores**: 4+ physical cores (for concurrent benchmarks)
- **Cooling**: Active cooling (prevents throttling)

**Check CPU Features**:
```bash
# Linux
lscpu | grep -i avx

# macOS
sysctl -a | grep -i avx

# Expected: avx2, avx512 (optional)
```

### Thermal Throttling

**Issue**: CPU throttles during benchmarks (skewed results)
**Detection**: Results vary significantly between runs (>10%)
**Solution**:
1. Ensure good cooling (active fan, room temperature <25°C)
2. Close background applications
3. Run benchmarks in shorter bursts (1-2 groups at a time)

---

## Reporting Results

### Quick Summary
```bash
# Generate summary report
cargo bench --bench capsule_hash64_bench 2>&1 | tee benchmark_results.txt

# Extract key metrics
grep "time:" benchmark_results.txt | awk '{print $1, $NF}'
```

### Detailed Report
```bash
# Open HTML report in browser
xdg-open target/criterion/index.html  # Linux
open target/criterion/index.html      # macOS

# Navigate to:
# - capsule_hash64_bench (benchmark group)
# - Individual benchmarks (click for detailed graphs)
# - Compare across runs (if multiple runs executed)
```

### CI/CD Integration
```yaml
# .github/workflows/benchmarks.yml
name: Benchmarks

on: [push, pull_request]

jobs:
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
          override: true
      - name: Run benchmarks
        run: |
          cargo bench --bench capsule_hash64_bench -- --output-format bencher | tee output.txt
      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'cargo'
          output-file-path: output.txt
```

---

## Next Steps (After Running Benchmarks)

1. ✅ **Validate Results**: Compare against expected results (see "Expected Results" section)
2. ✅ **Reality Check**: Verify SIMD speedup is 2-4× (NOT 10×)
3. ✅ **Document**: Copy benchmark results to Phase 3 completion report
4. ✅ **Commit**: Commit benchmark suite to repository
5. ✅ **CI/CD**: Set up continuous benchmarking (optional)

---

## References

- **B32 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`
- **UCE33 Analysis**: `/home/samuel/Primitives/clapi_core/UCE33_CAPSULE_HASH64_ANALYSIS.md`
- **Benchmark Delivery**: `/home/samuel/Primitives/clapi_core/B32_CAPSULE_HASH64_BENCHMARK_DELIVERY.md`
- **CapsuleHash64 Implementation**: `/home/samuel/Primitives/clapi_core/src/capsules/capsule_hash64.rs`

---

**Status**: ✅ Ready to run
**Benchmark Suite**: 14 benchmarks, 619 lines, B32-compliant
**Expected Time**: 5-7 minutes (scalar + SIMD)
**Output**: HTML report in `target/criterion/index.html`
