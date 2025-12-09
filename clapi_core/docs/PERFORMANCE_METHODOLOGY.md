# B32 Performance Methodology
# Hardware Specifications & Benchmarking Standards

**Date**: 2025-10-21
**Framework**: B32 Benchmark32 with Hardware Reality Checks (K1-K50)
**Project**: clapi_core Timeline Aggregation Capsule
**Purpose**: Enable reproducible performance validation

---

## Executive Summary

This document specifies the **exact hardware environment**, **compiler configuration**, and **measurement methodology** used for all performance benchmarks in clapi_core. Following these specifications ensures reproducible results and B32 framework compliance.

**Key Requirements**:
- Same hardware for all measurements
- Same compiler version and optimization flags
- Same workload patterns
- 95% confidence intervals (1000+ iterations)
- Fair baselines (no strawmen)

---

## Hardware Environment

### Test System Specifications

```
CPU: Intel Ultra 7 155H (14 cores: 6P + 8E + 2LP)
  - P-cores (Performance): 6 cores @ 5.1GHz max boost
    - Actual sustained: 4.8GHz (thermal throttling)
    - Use case: Burst workloads, single-threaded benchmarks
  - E-cores (Efficiency): 8 cores @ 4.0GHz max boost
    - Actual sustained: 3.6-3.8GHz
    - Use case: Background workers, parallel throughput
  - Logical processors: 22 threads total (6P + 8E + 8LP via hyperthreading)

RAM: 64GB DDR5-5600
  - Theoretical bandwidth: 89.6GB/s (dual-channel DDR5-5600)
  - Measured sequential: 15.2GB/s (17% of theoretical - B32 K3)
  - Measured random: 3-5GB/s (5% of theoretical - B32 K3)
  - Latency: 90-100ns (DRAM access - B32 K6)

Cache Hierarchy (B32 K6):
  - L1 Data: 48KB per P-core, 1ns latency
  - L2: 2MB per P-core, 3ns latency
  - L3: 24MB shared, 9-12ns latency
  - Cache line: 64 bytes (alignment critical for capsules)

Storage: NVMe SSD (not benchmarked - not relevant for in-memory capsules)

Cooling: Active cooling (65W sustained TDP)
  - Without cooling: Throttles after 30 seconds (B32 K21)
  - With cooling: Sustained 4.8GHz P-core boost
  - Impact: 40% performance difference (B32 K5)

OS: Linux 6.14.0-33-generic (Ubuntu)
  - Kernel scheduler: CFS (Completely Fair Scheduler)
  - NUMA: Single node (no cross-NUMA overhead)
  - Hugepages: 2MB hugepages available (not used by default)
```

### Hardware Reality Checks (B32 K1-K9)

The following measurements have been validated on this hardware:

| Reality Check | Theoretical | Measured | Ratio | B32 Reference |
|---------------|-------------|----------|-------|---------------|
| **K2** AtomicU64 CAS | N/A | 10-15ns | N/A | Hardware measurement |
| **K2** AtomicU64 FetchAdd | N/A | 20ns | N/A | Hardware measurement |
| **K2** AtomicU128 CAS | N/A | 15-25ns | N/A | Hardware measurement |
| **K3** Memory bandwidth (seq) | 89.6GB/s | 15.2GB/s | 17% | DDR5-5600 |
| **K3** Memory bandwidth (rand) | 89.6GB/s | 3-5GB/s | 5% | DDR5-5600 |
| **K4** Mutex uncontended | N/A | 30ns | N/A | std::Mutex |
| **K4** RwLock read uncontended | N/A | 25ns | N/A | std::RwLock |
| **K6** L1 cache latency | N/A | 1ns | N/A | 48KB per core |
| **K6** L2 cache latency | N/A | 3ns | N/A | 2MB per core |
| **K6** L3 cache latency | N/A | 9-12ns | N/A | 24MB shared |
| **K6** DRAM latency | N/A | 90-100ns | N/A | DDR5-5600 |

---

## Compiler Environment

### Rust Toolchain

```
Compiler: rustc 1.83.0-nightly (2025-10-21)
  - LLVM version: 19.1.0
  - Target triple: x86_64-unknown-linux-gnu
  - Host triple: x86_64-unknown-linux-gnu

Optimization Flags (--release):
  - opt-level: 3 (maximum optimization)
  - lto: "fat" (link-time optimization enabled)
  - codegen-units: 1 (maximize inlining)
  - debug: false (no debug info in release)
  - panic: "abort" (zero panic overhead)

RUSTFLAGS:
  - -C target-cpu=native (use Intel Ultra 7 155H instructions)
  - -C link-arg=-fuse-ld=lld (use LLVM linker - 30% faster builds)
  - -C embed-bitcode=no (reduce binary size)

Nightly Features (where applicable):
  - portable_simd (safe SIMD via std::simd)
  - const_fn_floating_point (compile-time FP math)
  - atomic_from_mut (zero-copy atomic views)
```

### Cargo Configuration

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
debug = false
strip = true
overflow-checks = false

[profile.bench]
inherits = "release"
debug = true  # Keep symbols for profiling
```

---

## Benchmarking Methodology

### B32 Framework Compliance (Guidelines 1-32)

All benchmarks MUST follow these B32 guidelines:

#### B1: Fair Baseline Selection

**Never compare against strawmen.**

```rust
// ❌ Bad: Strawman comparison
let naive_mutex = std::sync::Mutex::new(0);

// ✅ Good: Fair comparison
let parking_lot = parking_lot::Mutex::new(0);  // Optimized mutex
let dashmap = dashmap::DashMap::new();        // Industry standard concurrent map
let rwlock = std::sync::RwLock::new(0);       // Common pattern
```

**Multiple baselines required**:
- Baseline 1: Industry standard (DashMap, parking_lot::Mutex)
- Baseline 2: Common pattern (RwLock<HashMap>, std::Mutex)
- Baseline 3: Naive implementation (for context only)

#### B2: Statistical Rigor

**Use Criterion for all micro-benchmarks.**

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_timeline_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_append");

    // B2: Configure statistical rigor
    group.sample_size(1000);           // 1000+ iterations (B32 minimum)
    group.confidence_level(0.95);      // 95% confidence interval
    group.measurement_time(Duration::from_secs(10)); // 10s sustained measurement

    group.bench_function("append", |b| {
        let capsule = TimelineAggregationCapsuleCore::new(1440, 60).unwrap();
        b.iter(|| {
            capsule.append(black_box(1_634_567_890)).unwrap();
        });
    });

    group.finish();
}
```

**Required reporting**:
- P50, P95, P99 percentiles (not just mean)
- Standard deviation and variance
- 95% confidence intervals
- Sample size (n ≥ 1000)

#### B3: Realistic Workloads

**Test production-like scenarios.**

```rust
// ❌ Bad: Synthetic loop
for i in 0..1000000 {
    atomic.fetch_add(1, Ordering::Relaxed);
}

// ✅ Good: Realistic workload
for event in real_audit_events {
    capsule.append(event.timestamp).unwrap();
    capsule.flush_if_needed().unwrap();
    capsule.query_bucket(event.bucket_id).unwrap();
}
```

**Workload requirements**:
- Production data sizes (100K-1M events)
- Realistic access patterns (80/20 read/write)
- Actual concurrency levels (1-16 threads)
- Sustained testing (>60 seconds for thermal stability)

#### B4: Contention Scenarios

**Test both uncontended and contended cases.**

```rust
fn bench_contention_scaling(c: &mut Criterion) {
    for num_threads in [1, 2, 4, 8, 16] {
        c.bench_function(&format!("append_{}_threads", num_threads), |b| {
            let capsule = Arc::new(TimelineAggregationCapsuleCore::new(...).unwrap());
            b.iter(|| {
                let handles: Vec<_> = (0..num_threads)
                    .map(|_| {
                        let c = Arc::clone(&capsule);
                        thread::spawn(move || {
                            c.append(timestamp).unwrap();
                        })
                    })
                    .collect();

                for h in handles {
                    h.join().unwrap();
                }
            });
        });
    }
}
```

**Contention levels**:
- 1 thread: Uncontended baseline
- 2-4 threads: Light contention (typical)
- 8-12 threads: Moderate contention
- 16+ threads: Heavy contention (pathological)

#### B5: Full Reporting

**Comprehensive performance report template:**

```markdown
## Benchmark Results

Hardware: Intel Ultra 7 155H (6P+8E cores, 64GB DDR5-5600)
Compiler: rustc 1.83.0-nightly (LLVM 19.1.0)
OS: Linux 6.14.0-33-generic
Cooling: Active (65W sustained)

Workload: Timeline append (1M operations, 64B events)
Baseline: DashMap::insert (optimized concurrent hashmap)

Results (95% CI, n=1000):
--------------------------
Uncontended (1 thread):
  Baseline (DashMap):  500ns ± 20ns (P99: 800ns)
  Timeline:            78ns ± 5ns (P99: 120ns)
  Speedup:             6.4× faster

Light Contention (4 threads):
  Baseline (DashMap):  2µs ± 100ns (P99: 5µs)
  Timeline:            85ns ± 8ns (P99: 150ns)
  Speedup:             23.5× faster

Heavy Contention (16 threads):
  Baseline (DashMap):  10µs ± 500ns (P99: 20µs)
  Timeline:            95ns ± 12ns (P99: 200ns)
  Speedup:             105× faster

Variance: 6.4% (acceptable <15%)
Reproducibility: 3/3 runs consistent
```

---

## Performance Validation Standards

### Hardware Reality Checks (B32 K1-K50)

Before claiming any performance improvement, validate against these reality checks:

| Category | Reality Check | Threshold | Reference |
|----------|---------------|-----------|-----------|
| **Speedup Claims** | K27 Honest Gains | 10-50% typical, 2-10× exceptional | B32 K27 |
| **Atomic Operations** | K2 CAS Latency | 10-15ns (AtomicU64) | B32 K2 |
| **Memory Bandwidth** | K3 Sequential | 15.2GB/s measured | B32 K3 |
| **Synchronization** | K4 Mutex | 30ns uncontended | B32 K4 |
| **Cache Hierarchy** | K6 L1/L2/L3 | 1ns/3ns/12ns | B32 K6 |
| **SIMD Speedup** | K9 AVX2 | 3-4× typical, 8× theoretical | B32 K9 |
| **Allocation** | K13 Small Alloc | 20ns (<256B) | B32 K13 |
| **Tail Latency** | K43 Percentiles | P99.9 = 10-20× P50 | B32 K43 |

### Validation Checklist

Before reporting any benchmark result, ensure:

- [ ] Measured on actual Intel Ultra 7 155H hardware
- [ ] Sustained for >60 seconds (thermal stability - K21)
- [ ] With production-like data (not synthetic)
- [ ] Under thermal constraints (active cooling)
- [ ] With realistic concurrency (1-16 threads)
- [ ] Including allocation overhead (if any)
- [ ] With monitoring enabled (representative of production)
- [ ] Against fair baselines (DashMap, parking_lot, RwLock)
- [ ] With 95% confidence intervals (Criterion, n≥1000)
- [ ] Reproducible across 3+ independent runs

---

## Red Flags in Performance Claims

### Suspicious Claims (B32 K27)

**Immediate investigation required if:**

| Claim | Threshold | Action |
|-------|-----------|--------|
| Speedup > 10× | Exceptional | Extensive validation required |
| Speedup > 100× | Rare | Requires algorithm change proof |
| P99.9 > 20× P50 | Violates K43 | Investigate tail latency outliers |
| P99.99 > 100× P50 | Violates K43 | Profile GC, thermal, OS preemption |

### Common Measurement Errors

**Avoid these pitfalls:**

1. **Cherry-picking best runs** ❌
   - Report median of 3+ runs, not minimum
   - Include variance and outliers

2. **Ignoring thermal throttling** ❌
   - Sustained benchmarks (>60s) detect throttling
   - Monitor CPU frequency during test

3. **Comparing debug vs release** ❌
   - Always --release for both baseline and candidate
   - Never benchmark in debug mode

4. **Using theoretical instead of measured** ❌
   - K3: Use 15.2GB/s measured, not 89.6GB/s theoretical
   - K2: Use 10-15ns measured, not 5ns estimated

5. **Ignoring setup/teardown costs** ❌
   - Include allocation overhead in measurements
   - Amortize one-time costs over iterations

---

## Benchmark Execution Commands

### Running Benchmarks

```bash
# Standard benchmarks (10s per test)
cargo bench --bench timeline_integration_benchmarks

# Fair baseline comparison
cargo bench --bench e16_async_flush_validation

# Latency budget validation
cargo test --test latency_budget_validation --release

# Sustained 1-hour test (production validation)
cargo test --test latency_budget_validation --release -- --ignored test_sustained_1hour

# Generate Criterion HTML reports
cargo bench --bench timeline_integration_benchmarks -- --save-baseline main
# Reports saved to: target/criterion/report/index.html
```

### Profiling Commands

```bash
# CPU profiling with perf
perf record -F 99 --call-graph dwarf -- cargo bench --bench timeline_integration_benchmarks
perf report

# Memory bandwidth profiling
perf stat -e cycles,instructions,cache-references,cache-misses,L1-dcache-loads,L1-dcache-load-misses cargo bench

# Flamegraph generation
cargo flamegraph --bench timeline_integration_benchmarks

# Cache analysis (Intel VTune alternative)
valgrind --tool=cachegrind --cachegrind-out-file=cachegrind.out cargo bench
cg_annotate cachegrind.out
```

---

## Reproducibility Protocol

### Environment Setup

```bash
# 1. Fix CPU governor to performance mode
sudo cpufreq-set -g performance

# 2. Disable turbo boost (consistent frequency)
echo 0 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo

# 3. Disable hyperthreading (optional, for single-threaded tests)
echo 0 | sudo tee /sys/devices/system/cpu/cpu*/online

# 4. Set CPU affinity (pin to P-cores)
taskset -c 0-5 cargo bench  # P-cores only

# 5. Close background processes
# - Chrome/Firefox (memory pressure)
# - Docker (CPU interference)
# - System monitors (I/O contention)
```

### Validation Commands

```bash
# Verify CPU frequency
watch -n 1 "grep MHz /proc/cpuinfo | head -6"

# Monitor thermal throttling
sensors | grep -i cpu

# Check memory bandwidth (requires stream benchmark)
./stream_benchmark

# Verify no background interference
ps aux --sort=-%cpu | head -10
```

---

## Continuous Benchmarking (CI/CD)

### Benchmark Suite Structure

```
benches/
├── timeline_integration_benchmarks.rs  # B32-compliant fair baselines
├── e16_async_flush_validation.rs       # E16 claim validation
├── fair_comparison_comprehensive.rs    # DashMap vs RwLock baselines
└── production_stress_bench.rs          # 1-hour sustained test

tests/
├── latency_budget_validation.rs        # E20 B32 K43 compliance
└── ...
```

### CI/CD Pipeline

```yaml
# .github/workflows/benchmarks.yml
name: Performance Benchmarks

on:
  pull_request:
  push:
    branches: [main]

jobs:
  benchmarks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust nightly
        run: rustup toolchain install nightly --profile minimal

      - name: Run benchmarks
        run: cargo +nightly bench --all-features

      - name: Validate latency budgets
        run: cargo +nightly test --test latency_budget_validation --release

      - name: Check for performance regression
        run: cargo +nightly bench -- --save-baseline pr-${{ github.event.pull_request.number }}

      - name: Upload Criterion reports
        uses: actions/upload-artifact@v3
        with:
          name: criterion-reports
          path: target/criterion/
```

---

## Conclusion

This methodology ensures all clapi_core performance claims are:
- **Reproducible** (same hardware, compiler, workload)
- **Statistically valid** (95% CI, n≥1000)
- **Honestly reported** (fair baselines, no strawmen)
- **B32-compliant** (K1-K50 reality checks applied)

**Status**: ACTIVE - All benchmarks must follow this methodology

**Last Updated**: 2025-10-21
**Framework Version**: B32 v1.0 (50 Hardware Reality Checks)
**Validator**: Performance Expert
