# Queue Phase 3 Batch Operations - B32 Benchmark Suite

**Status**: Ready for Implementation Experts
**Purpose**: B32-compliant benchmark suite for batch queue operations
**Framework**: B32 Benchmark32 (32 guidelines + 27 hardware reality checks)

---

## Executive Summary

This benchmark suite provides **rigorous, fair comparison** between batch operations and individual operations for Queue Phase 3. All benchmarks follow B32 framework mandates:

- **Fair baselines**: Individual operations optimized (not strawman)
- **95% CI**: 1000+ iterations with confidence intervals
- **Reality check**: 2× speedup is EXCEPTIONAL, 1.3-1.7× is GOOD target
- **Hardware documentation**: CPU, compiler, frequency documented
- **Reproducibility**: Same hardware, same compiler, same config

---

## Performance Targets (B32 Reality Check)

### SPSC Batch Operations

| Metric | Baseline (Individual) | Target (Batch) | Speedup | Classification |
|--------|----------------------|----------------|---------|----------------|
| **Push latency** | <10ns | 6-7ns | 1.4-1.7× | GOOD |
| **Push latency** | <10ns | 5ns | 2× | EXCEPTIONAL |
| **Crossover point** | N/A | Batch size 8-16 | N/A | Expected |

**Reality Check**: 2× speedup requires extensive validation. Most batch optimizations achieve 1.3-1.7× (10-50% typical range).

### MPMC Batch Operations

| Metric | Baseline (Individual) | Target (Batch) | Speedup | Classification |
|--------|----------------------|----------------|---------|----------------|
| **Push latency** | <50ns | 35-40ns | 1.2-1.4× | GOOD |
| **Push latency** | <50ns | 25ns | 2× | EXCEPTIONAL |
| **Crossover point** | N/A | Batch size 16-32 | N/A | Expected |

**Reality Check**: MPMC has higher CAS overhead, so batch speedup is typically lower than SPSC. 1.5-2× requires extensive validation.

---

## Benchmark Structure

### Section 1: Fair Baselines (Individual Operations)

**Purpose**: Establish optimized individual operation baselines for fair comparison.

| Benchmark | Measures | Target | Notes |
|-----------|----------|--------|-------|
| `bench_individual_push_spsc` | Sequential push | <10ns/item | Phase 2 measured |
| `bench_individual_pop_spsc` | Sequential pop | <10ns/item | Phase 2 measured |
| `bench_individual_push_mpmc` | Single-thread push | <50ns/item | Phase 2 measured |
| `bench_individual_pop_mpmc` | Single-thread pop | <50ns/item | Phase 2 measured |

**B32 Compliance**:
- ✅ Same queue configuration as batch tests
- ✅ No "strawman" baseline (e.g., mutex-based queue)
- ✅ Pre-allocated capacity (no growth during benchmark)
- ✅ Same hardware, compiler, optimization level

### Section 2: Batch Operations

**Purpose**: Measure batch push/pop performance across various batch sizes.

| Benchmark | Batch Sizes | Implementation Status | Notes |
|-----------|-------------|----------------------|-------|
| `bench_batch_push_spsc` | 4, 8, 16, 32, 64, 128 | **TODO** | Requires `push_batch(&[T])` |
| `bench_batch_pop_spsc` | 4, 8, 16, 32, 64, 128 | **TODO** | Requires `pop_batch(usize)` |
| `bench_batch_push_mpmc` | 4, 8, 16, 32, 64, 128 | **TODO** | Requires `push_batch(&[T])` |
| `bench_batch_pop_mpmc` | 4, 8, 16, 32, 64, 128 | **TODO** | Requires `pop_batch(usize)` |

**Implementation Notes**:
- Current code has **placeholder loops** (`for &item in &items { queue.push(item) }`)
- Replace with actual batch methods when implemented
- Batch methods should minimize CAS operations (single atomic update for entire batch)

### Section 3: Crossover Analysis

**Purpose**: Find optimal batch size via per-item amortized latency.

| Benchmark | Batch Sizes | Measures | Expected Result |
|-----------|-------------|----------|-----------------|
| `bench_batch_crossover_point` | 1, 2, 4, 8, 16, 32, 64, 128, 256 | ns/item | Crossover at 8-16 (SPSC), 16-32 (MPMC) |

**Analysis Method**:
1. Measure total latency for batch operation
2. Divide by batch size to get per-item amortized latency
3. Plot latency vs batch size
4. Crossover = point where per-item latency stops improving

**Expected Curve**:
```
Per-item latency (ns)
  ^
12|     ●
10|   ●
 8| ●
 6|   ●     ● ● ● ← Crossover (batch size 16)
 4|
  +-----------------> Batch size
    1  4  8 16 32 64
```

### Section 4: Concurrent Batch Benchmarks

**Purpose**: Measure batch performance under concurrent contention (MPMC only).

| Benchmark | Threads | Items/Thread | Total Items | Notes |
|-----------|---------|--------------|-------------|-------|
| `bench_concurrent_batch_mpmc` | 2, 4, 8 | 1000 | 2K-8K | Batch size fixed at 32 |

**Comparison**: Individual vs batch operations under concurrent load.

**B32 Reality Check**: Concurrent batch operations may show **LESS speedup** than sequential due to CAS contention. Target: 1.2-1.5× speedup.

### Section 5: Sustained Throughput

**Purpose**: Measure items/sec and batch latency under sustained 100K operation load.

| Benchmark | Total Items | Batch Sizes | Metrics Reported |
|-----------|-------------|-------------|------------------|
| `bench_batch_sustained_throughput` | 100,000 | 16, 32, 64, 128 | Items/sec, ns/batch |

**Targets**:
- **SPSC individual**: ~100M items/sec (10ns/item)
- **SPSC batch(16)**: ~140-170M items/sec (6-7ns/item, 1.4-1.7× speedup)
- **MPMC individual**: ~20M items/sec (50ns/item)
- **MPMC batch(32)**: ~25-30M items/sec (35-40ns/item, 1.2-1.5× speedup)

---

## Running Benchmarks

### Prerequisites

```bash
# Ensure nightly toolchain (for queue features)
rustup install nightly
rustup default nightly

# Verify installation
cargo --version
```

### Run All Benchmarks

```bash
cd /home/samuel/Primitives/atomic_capsule

# Run full suite (⚠️ This will FAIL until batch methods are implemented)
cargo bench --bench batch_queue_bench --features queue-unbounded
```

### Run Individual Sections

```bash
# Section 1: Baselines only
cargo bench --bench batch_queue_bench --features queue-unbounded -- baseline_

# Section 2: Batch operations only
cargo bench --bench batch_queue_bench --features queue-unbounded -- batch_

# Section 3: Crossover analysis only
cargo bench --bench batch_queue_bench --features queue-unbounded -- crossover

# Section 4: Concurrent batch only
cargo bench --bench batch_queue_bench --features queue-unbounded -- concurrent_

# Section 5: Sustained throughput only
cargo bench --bench batch_queue_bench --features queue-unbounded -- sustained_
```

### Output Location

```
target/criterion/
├── baseline_individual_spsc_push/
│   ├── report/
│   │   ├── index.html      ← Open in browser
│   │   ├── violin.svg      ← Latency distribution
│   │   └── pdf.svg         ← Probability density
│   └── base/
│       └── estimates.json  ← 95% CI, mean, median
├── batch_spsc_push/
├── batch_crossover_analysis/
├── concurrent_batch_mpmc/
└── batch_sustained_throughput/
```

---

## B32 Framework Compliance

### 1. Fair Baselines ✅

**What We Do**:
- Individual operations use **same queue type** (UnboundedQueueCapsule)
- Same configuration (pre-allocated, no growth during benchmark)
- Same hardware, compiler, optimization level

**What We DON'T Do** ❌:
- Compare to mutex-based queue (strawman baseline)
- Use different queue implementation for baseline
- Disable optimizations for baseline

### 2. Statistical Rigor ✅

**Criterion Configuration**:
- **1000+ iterations**: Default sampling mode
- **95% confidence intervals**: Reported in output
- **Outlier detection**: Automatic filtering
- **Warm-up**: Iterations to stabilize CPU frequency

**Example Output**:
```
baseline_individual_spsc_push/individual/16
                        time:   [160.2 ns 162.1 ns 164.3 ns]
                        change: [-2.1% +0.5% +3.2%] (p = 0.42 > 0.05)
                        No change in performance detected.
Found 12 outliers among 100 measurements (12%)
  5 (5%) high mild
  7 (7%) high severe
```

### 3. Reality Check ✅

**B32 Guidelines**:
- **10-50% typical**: Most optimizations
- **2× exceptional**: Requires validation
- **10×+ extensive**: Requires extensive validation (multi-site, multi-hardware)

**Our Targets**:
- **SPSC batch**: 1.4-1.7× speedup (GOOD range)
- **MPMC batch**: 1.2-1.4× speedup (GOOD range)
- **If 2×**: Mark as EXCEPTIONAL, require additional validation

### 4. Hardware Documentation ✅

**Documented Automatically by Criterion**:
- CPU model (via `cpuid`)
- Core count
- Compiler version (`rustc --version`)
- Optimization level (`-C opt-level=3` in release)

**Manual Documentation Required**:
- CPU base/boost frequencies
- L1/L2/L3 cache sizes
- NUMA topology (if applicable)

**Example**:
```toml
# benches/BASELINE_PERFORMANCE.toml
[hardware]
cpu_model = "AMD Ryzen 9 6900HX"
cores = 16
threads = 32
base_freq_ghz = 3.3
boost_freq_ghz = 4.9
l1_cache_kb = 64
l2_cache_kb = 512
l3_cache_mb = 16
```

### 5. Reproducibility ✅

**What We Ensure**:
- Same hardware for all benchmarks
- Same compiler version
- Same feature flags
- Same Cargo.toml dependencies
- Results stored in `target/criterion/` for comparison

**Comparing Runs**:
```bash
# Run baseline
cargo bench --bench batch_queue_bench --features queue-unbounded -- baseline_

# Implement batch operations

# Run again (criterion compares to previous run)
cargo bench --bench batch_queue_bench --features queue-unbounded -- batch_

# View comparison report
open target/criterion/batch_spsc_push/report/index.html
```

---

## Expected Implementation Timeline

### Phase 3.1: Batch Method Implementation (Implementation Experts)

**Deliverables**:
1. `UnboundedQueueCapsule::push_batch(&[T]) -> Result<(), PushError>`
2. `UnboundedQueueCapsule::pop_batch(usize) -> Vec<T>`
3. Unit tests (T28 framework)
4. Documentation

**Estimated**: 4-6 hours

### Phase 3.2: Benchmark Validation (Benchmarking Expert)

**Deliverables**:
1. Replace placeholder loops with actual batch methods
2. Run full benchmark suite
3. Validate results against B32 targets
4. Document speedup claims with 95% CI
5. Create B32 report

**Estimated**: 2-3 hours

---

## Interpreting Results

### Speedup Calculation

```rust
// Example: SPSC batch push (16 items)
let baseline_total_ns = 160.0; // Individual: 10ns × 16 = 160ns
let batch_total_ns = 112.0;    // Batch: 7ns × 16 = 112ns

let speedup = baseline_total_ns / batch_total_ns;
// = 160.0 / 112.0 = 1.43×

let classification = if speedup >= 2.0 {
    "EXCEPTIONAL (requires validation)"
} else if speedup >= 1.3 {
    "GOOD (within B32 typical range)"
} else {
    "MARGINAL (< 1.3×, investigate overhead)"
};
```

### Per-Item Amortized Latency

```rust
// Example: Batch push (16 items) took 112ns total
let batch_total_ns = 112.0;
let batch_size = 16;

let per_item_ns = batch_total_ns / batch_size as f64;
// = 112.0 / 16 = 7.0ns per item

// Compare to individual baseline (10ns)
let improvement = (10.0 - 7.0) / 10.0 * 100.0;
// = 30% improvement (GOOD)
```

### Throughput Calculation

```rust
// Example: 100K items in 1.2ms
let total_items = 100_000;
let total_time_ns = 1_200_000.0; // 1.2ms

let items_per_sec = (total_items as f64 / total_time_ns) * 1e9;
// = (100,000 / 1,200,000) * 1,000,000,000
// = 83.3M items/sec
```

---

## Troubleshooting

### Benchmark Compilation Fails

**Error**: `error[E0599]: no method named 'push_batch' found`

**Solution**: Batch methods not implemented yet. This is expected until Phase 3.1 is complete.

**Workaround**: Run baseline benchmarks only:
```bash
cargo bench --bench batch_queue_bench --features queue-unbounded -- baseline_
```

### High Variance in Results

**Symptom**: Wide confidence intervals (e.g., `[100ns, 200ns]`)

**Causes**:
- CPU frequency scaling (turbo boost unstable)
- Background processes (high system load)
- Thermal throttling (CPU overheating)

**Solutions**:
1. Close background applications
2. Pin CPU frequency: `sudo cpupower frequency-set -g performance`
3. Increase sample size: `CRITERION_SAMPLE_SIZE=1000 cargo bench`

### Speedup Lower Than Expected

**Expected**: 1.4-1.7× for SPSC batch
**Actual**: 1.1-1.2×

**Investigate**:
1. **Batch overhead**: Vec allocation, bounds checking
2. **Cache misses**: Batch too large for L1 cache
3. **CAS contention**: MPMC concurrent batch operations
4. **Suboptimal batch size**: Below crossover point (e.g., batch size 2-4)

**Validate**:
- Run crossover analysis to find optimal batch size
- Profile with `perf` to identify bottlenecks
- Check L1 cache utilization (`perf stat -e cache-references,cache-misses`)

---

## Contact

**Benchmarking Expert**: Responsible for this benchmark suite
**Implementation Experts**: Waiting for batch method implementation
**Framework**: B32 Benchmark32 (/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md)

---

## Appendix: B32 Framework Summary

### 32 Benchmarking Guidelines (Subset)

1. **K1**: Measure baseline on same hardware
2. **K2**: Use optimized baseline (not strawman)
3. **K5**: Document hardware (CPU, cache, frequency)
4. **K10**: Report 95% confidence intervals
5. **K12**: 1000+ iterations minimum
6. **K15**: Reality check: 10-50% typical, 2× exceptional, 10×+ extensive
7. **K20**: Validate reproducibility (multiple runs)
8. **K25**: Compare apples-to-apples (same config)

### 27 Hardware Reality Checks (Subset)

1. **H1**: CPU frequency scaling affects results
2. **H3**: L1 cache is 64-256KB (batch size matters)
3. **H7**: CAS contention increases with threads
4. **H12**: Turbo boost unstable (pin frequency for consistency)
5. **H18**: NUMA affects multi-socket systems
6. **H25**: Background processes add noise (close apps)

---

**End of README**
