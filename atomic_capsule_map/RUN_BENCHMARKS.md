# Quick Start: Running B32 Benchmark Suite

**Target**: AtomicCapsuleMap v1.0 comprehensive benchmarks
**Time**: 30-60 minutes for full suite
**Framework**: B32 (32 guidelines + 27 hardware checks)

---

## Quick Commands

```bash
# Go to project directory
cd /home/samuel/Primitives/atomic_capsule_map

# Run full benchmark suite
cargo bench --bench v1_comprehensive

# View results
firefox target/criterion/report/index.html
```

---

## Detailed Instructions

### 1. System Preparation (Optional but Recommended)

```bash
# Fix CPU frequency for consistent results
sudo cpupower frequency-set -g performance

# Verify hardware
lscpu | grep -E "Model name|MHz|cache"

# Close unnecessary applications
# Aim for <5% CPU usage before starting
```

### 2. Verify Build

```bash
# Test compilation
cargo build --release --lib
cargo bench --bench v1_comprehensive --no-run
```

Expected output: `Finished bench profile [optimized] target(s)`

### 3. Run Benchmarks

#### Full Suite (Recommended)

```bash
# Run all 18 benchmarks (~30-60 minutes)
cargo bench --bench v1_comprehensive
```

Sections executed:
1. Micro-benchmarks (5 benchmarks)
2. Operation benchmarks (5 benchmarks)
3. Contention benchmarks (3 benchmarks)
4. Comparison benchmarks (2 benchmarks)
5. Hardware benchmarks (3 benchmarks)

#### Specific Sections (Faster)

```bash
# Micro-benchmarks only (~5-10 min)
cargo bench --bench v1_comprehensive micro

# Operations only (~10-15 min)
cargo bench --bench v1_comprehensive operations

# Comparisons vs DashMap (~10 min)
cargo bench --bench v1_comprehensive comparison

# Hardware reality checks (~5-10 min)
cargo bench --bench v1_comprehensive hardware
```

### 4. View Results

```bash
# Open HTML report
firefox target/criterion/report/index.html

# Or view raw data
cat target/criterion/*/new/estimates.json | jq .
```

### 5. Save Baseline

```bash
# Save results for future comparison
cargo bench --bench v1_comprehensive -- --save-baseline v1.0

# Later, compare against baseline
cargo bench --bench v1_comprehensive -- --baseline v1.0
```

---

## Expected Results (v0.1.1 Baseline)

Based on `DASHMAP_COMPARISON.md`:

| Benchmark | Expected Result | Interpretation |
|-----------|-----------------|----------------|
| `operations/insert` | ~361ns | **SLOW** - DashMap 9.9× faster |
| `operations/get` | ~8-12ns | **FAST** - 2.2× faster than DashMap |
| `operations/update` | ~32ns | **SLOW** - DashMap 1.9× faster |
| `comparison/vs_dashmap/get` | ACM wins | Lockfree read advantage |
| `comparison/vs_dashmap/insert` | DashMap wins | Allocation overhead |
| `contention/read_scaling/8threads` | Similar | Both show contention |

**Key Finding**: INSERT performance is the critical bottleneck (361ns vs 36ns for DashMap)

---

## Profiling INSERT (Critical Path)

### Flamegraph (Visual Profiling)

```bash
# Install if needed
cargo install flamegraph

# Generate flamegraph for INSERT
cargo flamegraph --bench v1_comprehensive -- operations/insert

# View flamegraph.svg in browser
firefox flamegraph.svg
```

Expected: Allocation overhead (>80% of time)

### CPU Profiling with Perf

```bash
# Record profile
perf record -g cargo bench --bench v1_comprehensive -- operations/insert

# View report
perf report
```

Look for:
- Allocation functions (malloc, __rust_alloc)
- Atomic operations (CAS loops)
- Hash function overhead

### Memory Profiling

```bash
# Install valgrind if needed
sudo apt install valgrind

# Profile memory
valgrind --tool=massif cargo bench --bench v1_comprehensive -- operations/insert

# View results
ms_print massif.out.*
```

Look for:
- Peak memory usage
- Allocation patterns
- Memory fragmentation

### Hardware Counters

```bash
# Measure hardware events
perf stat -e instructions,cycles,L1-dcache-load-misses,cache-references,branches,branch-misses \
    cargo bench --bench v1_comprehensive -- operations/insert
```

Expected metrics:
- CPI (cycles per instruction): Should be <2.0
- Cache miss rate: Should be <5%
- Branch misprediction: Should be <3%

---

## Troubleshooting

### Benchmark Runs Too Long

```bash
# Reduce sample size (less statistical rigor)
# Edit benches/v1_comprehensive.rs:
# .sample_size(1000) → .sample_size(100)
```

### Permission Errors for perf/cpupower

```bash
# Add user to perf group
sudo usermod -a -G perf $USER

# Or run with sudo (not recommended)
sudo cargo bench --bench v1_comprehensive
```

### Criterion Warnings

```bash
# Update Criterion if outdated
cargo update -p criterion

# Clear old baselines
rm -rf target/criterion/
```

### Build Errors

```bash
# Clean and rebuild
cargo clean
cargo build --release --lib
cargo bench --bench v1_comprehensive --no-run
```

---

## Next Steps After Running Benchmarks

### 1. Compare Results

Compare your results with baseline in `DASHMAP_COMPARISON.md`:

- GET operations: Should be ~8-12ns (2× faster than DashMap)
- INSERT operations: Should be ~361ns (9.9× slower than DashMap)
- Concurrent reads: Should be competitive at low thread counts

### 2. Identify Bottlenecks

Focus on INSERT profiling:

```bash
# Generate flamegraph
cargo flamegraph --bench v1_comprehensive -- operations/insert

# Expected bottleneck: Allocation overhead (>80%)
```

### 3. Update Documentation

After analysis, update:
- `DASHMAP_COMPARISON.md` - Add v1.0 results
- `PERFORMANCE_VALIDATION.md` - Document improvements
- `V1_0_B32_FINAL_DELIVERY.md` - Update status

### 4. Optimize (v1.0 Goal)

Target improvements:
- INSERT: 361ns → <100ns (3.6× faster)
- Use bump allocator or arena for allocations
- Reduce atomic coordination overhead

---

## Understanding the Results

### Criterion Output Format

```
operations/insert   time:   [361.88 ns 362.45 ns 363.12 ns]
                    change: [-0.5% +0.2% +1.1%] (p = 0.32 > 0.05)
```

- **time**: [lower bound, point estimate, upper bound] at 95% confidence
- **change**: Comparison with previous run (if exists)
- **p-value**: Statistical significance (p < 0.05 = significant change)

### Interpreting Performance

**Good Performance**:
- GET: <15ns (lockfree read advantage)
- Low variance: ±1-2ns
- Linear scaling: 2× threads → 2× throughput

**Bad Performance**:
- INSERT: >300ns (allocation overhead)
- High variance: ±50ns+ (contention)
- Sublinear scaling: 2× threads → 1.5× throughput

### B32 Compliance Checks

After benchmarks complete, verify:
- ✅ 95% confidence intervals reported (Criterion automatic)
- ✅ Outliers detected (<15% typical)
- ✅ Multiple runs consistent (variance <10%)
- ✅ Warmup period sufficient (no trend in early samples)

---

## Reference

- **B32 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`
- **Baseline Results**: `DASHMAP_COMPARISON.md`
- **Full Compliance Report**: `B32_COMPLIANCE_REPORT.md`
- **Delivery Summary**: `V1_0_B32_FINAL_DELIVERY.md`

---

**Status**: ✅ Ready to run
**Estimated Time**: 30-60 minutes full suite, 5-10 minutes per section
**Critical Path**: Profile INSERT operation after benchmark completion
