# Phase 5: Runtime CPU Dispatch Benchmarks - Quick Start Guide

## Overview

This directory contains **6 B32-compliant benchmarks** for validating runtime CPU dispatch in kindly_dedup. These benchmarks measure:

1. **CPU detection overhead** (<1μs one-time)
2. **Dispatch overhead** (<10ns per call)
3. **AVX2 vs scalar speedup** (2-4× target)
4. **Portable binary regression** (<5% vs native)
5. **Throughput scaling** (60K+ docs/sec sustained)
6. **Multi-threaded contention** (linear to 12 threads)

**Total runtime**: ~15-20 minutes for all benchmarks

---

## Quick Start (5 Minutes)

### Run All Benchmarks

```bash
# From kindly_dedup directory
cargo bench --bench p5_runtime_dispatch

# View results in browser
open target/criterion/report/index.html
```

### Run Specific Benchmark

```bash
# Benchmark 1: CPU detection init
cargo bench --bench p5_runtime_dispatch cpu_capability_init

# Benchmark 2: Dispatch overhead
cargo bench --bench p5_runtime_dispatch dispatch_overhead

# Benchmark 3: AVX2 vs scalar
cargo bench --bench p5_runtime_dispatch avx2_vs_scalar_signature

# Benchmark 4: Portable vs native
cargo bench --bench p5_runtime_dispatch end_to_end_portable

# Benchmark 5: Throughput
cargo bench --bench p5_runtime_dispatch throughput_scaling

# Benchmark 6: Multi-threaded
cargo bench --bench p5_runtime_dispatch dispatch_contention
```

---

## Platform-Specific Testing

### Linux x86-64 AVX2 (Primary)

```bash
# Native AVX2 (best performance)
cargo bench --bench p5_runtime_dispatch

# Expected:
# - AVX2 path active
# - 2-4× speedup vs scalar
# - <10ns dispatch overhead
```

### Linux x86-64 SSE4.2 (Older CPUs)

```bash
# Disable AVX2, use SSE4.2
RUSTFLAGS="-C target-feature=-avx2" \
  cargo bench --bench p5_runtime_dispatch

# Expected:
# - SSE4.2 path active
# - 1.5-2× speedup vs scalar
# - Same dispatch overhead
```

### Linux x86-64 Scalar (Legacy/ARM)

```bash
# Disable all SIMD
RUSTFLAGS="-C target-feature=-avx2,-sse4.2" \
  cargo bench --bench p5_runtime_dispatch

# Expected:
# - Scalar path (baseline)
# - No SIMD speedup (1×)
# - Same dispatch overhead
```

### macOS Intel (Auto-detect)

```bash
# Automatic CPU feature detection
cargo bench --bench p5_runtime_dispatch

# Expected:
# - AVX2 (2013+ Macs) or SSE4.2 (2009+ Macs)
# - Auto-detection working correctly
```

### macOS ARM (M1/M2/M3)

```bash
# ARM NEON or scalar fallback
cargo bench --bench p5_runtime_dispatch

# Expected:
# - Scalar fallback (NEON support TBD)
# - No crashes ("Illegal instruction")
# - Correct results
```

### GCP Cloud Shell (SSE4.2 typical)

```bash
# Run in GCP Cloud Shell
cargo bench --bench p5_runtime_dispatch

# Expected:
# - SSE4.2 detection (typical GCP VMs)
# - 1.5-2× speedup
# - Same correctness
```

---

## Interpreting Results

### Benchmark 1: CPU Capability Init

**Good**:
```
cpu_capability_init/cold_init
  Time: 850 ns  (P50: 820 ns, P95: 1.2 μs, P99: 1.5 μs)

cpu_capability_init/cached_access
  Time: 6 ns  (P50: 5 ns, P95: 8 ns, P99: 12 ns)
```

**Why good**: Init <1μs (amortized), cached access <10ns

**Bad**:
```
cpu_capability_init/cached_access
  Time: 45 ns  (P50: 42 ns, P95: 58 ns, P99: 75 ns)
```

**Why bad**: >10ns cached access → Cache miss or contention

---

### Benchmark 2: Dispatch Overhead

**Good**:
```
dispatch_overhead/direct_scalar_call
  Time: 120 ns  (P50: 118 ns, P95: 135 ns)

dispatch_overhead/dispatched_call
  Time: 128 ns  (P50: 125 ns, P95: 142 ns)

Overhead: 8 ns (6.7%)
```

**Why good**: <10ns overhead, <10% of total

**Bad**:
```
dispatch_overhead/direct_scalar_call
  Time: 120 ns

dispatch_overhead/dispatched_call
  Time: 195 ns

Overhead: 75 ns (62.5%)
```

**Why bad**: >10ns overhead → Branch misprediction or cache miss

---

### Benchmark 3: AVX2 vs Scalar

**Good**:
```
avx2_vs_scalar_signature/scalar/100tokens
  Time: 8.5 μs  (P50: 8.2 μs, P95: 9.1 μs)
  Throughput: 117K signatures/sec

avx2_vs_scalar_signature/avx2/100tokens
  Time: 2.8 μs  (P50: 2.7 μs, P95: 3.2 μs)
  Throughput: 357K signatures/sec
  Speedup: 3.04×
```

**Why good**: 2-4× range (conservative SIMD speedup)

**Bad**:
```
Speedup: 1.2×  (too low - vectorization not working)
Speedup: 8.5×  (too high - suspicious, verify fairness)
```

---

### Benchmark 4: Portable vs Native

**Good**:
```
end_to_end_portable/portable/1000docs
  Time: 16.5 ms  (P50: 16.2 ms, P95: 17.8 ms)
  Throughput: 60,606 docs/sec

end_to_end_portable/native/1000docs
  Time: 16.2 ms  (P50: 15.9 ms, P95: 17.4 ms)
  Throughput: 61,728 docs/sec

Regression: 1.85%
```

**Why good**: <5% regression (acceptable for portability)

**Bad**:
```
Regression: 12.3%  (too high - dispatch overhead excessive)
```

---

### Benchmark 5: Throughput Scaling

**Good**:
```
throughput_scaling/sustained/10000docs
  Time: 165 ms  (P50: 163 ms, P95: 172 ms)
  Throughput: 60,606 docs/sec

Sustained over 10 seconds: ✅
vs Python baseline (1,572 docs/sec): 38.5×
```

**Why good**: ≥60K docs/sec sustained, maintains 30-50× claim

**Bad**:
```
Throughput: 42,000 docs/sec  (below 60K target)
Variance: 25%  (unstable performance)
```

---

### Benchmark 6: Multi-threaded Contention

**Good**:
```
dispatch_contention/threads/1_threads
  Throughput: 60K docs/sec
  Efficiency: 100% (baseline)

dispatch_contention/threads/4_threads
  Throughput: 230K docs/sec
  Speedup: 3.83×
  Efficiency: 95.8%

dispatch_contention/threads/8_threads
  Throughput: 440K docs/sec
  Speedup: 7.33×
  Efficiency: 91.6%

dispatch_contention/threads/12_threads
  Throughput: 600K docs/sec
  Speedup: 10.0×
  Efficiency: 83.3%
```

**Why good**: Near-linear to 12 threads (>80% efficiency)

**Bad**:
```
dispatch_contention/threads/8_threads
  Speedup: 3.2×  (40% efficiency - contention or CAS storms)
```

---

## Troubleshooting

### Problem: "Illegal instruction" crash

**Cause**: SIMD code running on unsupported CPU

**Fix**:
```bash
# Force scalar fallback
RUSTFLAGS="-C target-feature=-avx2,-sse4.2" \
  cargo bench --bench p5_runtime_dispatch
```

---

### Problem: Dispatch overhead >10ns

**Possible causes**:
1. Cache miss (singleton not initialized)
2. Branch misprediction (unlikely - CPU tier is constant)
3. Contention (multiple threads accessing singleton)

**Debug**:
```bash
# Check warmup period
cargo bench --bench p5_runtime_dispatch dispatch_overhead -- --warm-up-time 10

# Increase sample size
cargo bench --bench p5_runtime_dispatch dispatch_overhead -- --sample-size 10000
```

---

### Problem: AVX2 speedup <2×

**Possible causes**:
1. AVX2 not available (check `cat /proc/cpuinfo | grep avx2`)
2. Alignment issues (SIMD requires 32-byte alignment)
3. Small token counts (<64 elements, vectorization overhead dominates)

**Debug**:
```bash
# Verify CPU features
rustc --print cfg | grep target_feature

# Run only large documents (1000 tokens)
cargo bench --bench p5_runtime_dispatch avx2_vs_scalar_signature/1000tokens
```

---

### Problem: Throughput <60K docs/sec

**Possible causes**:
1. Thermal throttling (K21 - check CPU temperature)
2. Background processes (close other applications)
3. Swap usage (check `free -h`, need ≥4GB free RAM)

**Debug**:
```bash
# Monitor CPU during benchmark
watch -n 1 'cat /proc/cpuinfo | grep MHz'

# Check thermal throttling
watch -n 1 sensors

# Disable turbo boost (for consistent results)
echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo
```

---

### Problem: Multi-threaded efficiency <80%

**Possible causes**:
1. Memory bandwidth saturation (K29 - 15.2 GB/s limit)
2. False sharing (writers on same cache line)
3. CAS storms (excessive contention)

**Debug**:
```bash
# Reduce thread count (test 1-6 threads only)
cargo bench --bench p5_runtime_dispatch dispatch_contention/threads/4_threads

# Profile with perf
perf stat -e cache-misses,cache-references cargo bench ...
```

---

## Performance Targets Summary

| Metric | Target | Acceptance Criteria |
|--------|--------|---------------------|
| **CPU detection init** | <1μs | One-time cost, amortized over millions of calls |
| **Dispatch overhead** | <10ns | Cached atomic load + branch prediction |
| **AVX2 speedup** | 2-4× | Conservative SIMD speedup (K9 reality) |
| **Portable regression** | <5% | Acceptable tradeoff for single binary |
| **Throughput** | 60K+ docs/sec | Sustained performance over 10 seconds |
| **Thread scaling** | Linear to 12 | >80% efficiency @ 12 threads |

---

## B32 Framework Compliance

### ✅ Fair Baselines (B1)
- Scalar path (always available, NOT strawman)
- SSE4.2 path (mid-tier SIMD)
- AVX2 path (high-tier SIMD)
- Same hardware for all measurements

### ✅ Statistical Rigor (B2)
- 1000+ iterations (Criterion default)
- 95% confidence intervals
- 3-second warmup period
- Multiple independent runs

### ✅ Real Workloads (B3)
- Production-like LLM documents
- Realistic token distributions
- Full deduplication pipeline
- Sustained performance (10-second measurements)

### ✅ Contention Scenarios (B4)
- Uncontended (1 thread)
- Light contention (2-4 threads)
- Heavy contention (8-16 threads)
- 100% lockfree coordination

### ✅ Reporting Standards (B5)
- P50, P95, P99 percentiles
- Hardware specifications
- Compiler version and flags
- Thermal conditions
- Reproducibility instructions

---

## Expected Results (Reference)

Based on kindly_dedup CLAUDE.md and B32 framework:

### Single-threaded Performance
- **Scalar**: 60K docs/sec (baseline)
- **SSE4.2**: 90-120K docs/sec (1.5-2× speedup)
- **AVX2**: 120-240K docs/sec (2-4× speedup)

### Multi-threaded Performance (16 cores)
- **Ideal**: 960K docs/sec (16× linear)
- **Expected**: 768K docs/sec (12.8×, 80% efficiency)
- **Memory bandwidth limit**: K29 15.2 GB/s may saturate @ 12-14 threads

### Dispatch Overhead
- **One-time init**: <1μs (amortized <0.001ns per call)
- **Cached access**: <10ns (K2 atomic load)
- **Total overhead**: <0.1% of processing time

---

## Next Steps After Benchmarking

1. **Fill in p5_RESULTS.md** with actual measurements
2. **Validate all 6 targets** (CPU init, dispatch, speedup, regression, throughput, scaling)
3. **Test on all 6 platforms** (Linux AVX2/SSE4.2/Scalar, macOS Intel/ARM, GCP)
4. **Regression analysis** (<5% acceptable for portability)
5. **Production decision** (Ship if all targets met)

---

## Contact

- **Phase 5 Owner**: Claude (Benchmarking Expert)
- **Framework**: B32 Benchmark32 (Fair, Statistical, Real)
- **Documentation**: `/home/samuel/Primitives/kindly_dedup/PHASE5_TESTING_PLAN.md`

---

**Document**: `/home/samuel/Primitives/kindly_dedup/benches/P5_BENCHMARK_README.md`
**Status**: Production-Ready
**Last Updated**: 2025-11-02
