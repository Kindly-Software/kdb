# Arc<T> Benchmark Quick Start Guide

## TL;DR

```bash
# Setup (one-time)
sudo cpupower frequency-set -g performance

# Run all Arc benchmarks
cargo bench --bench arc_ops

# View results
firefox target/criterion/report/index.html
```

---

## Detailed Instructions

### 1. System Preparation (One-Time Setup)

```bash
# Disable CPU frequency scaling
sudo cpupower frequency-set -g performance

# Verify setting
cpupower frequency-info

# Check no background load
uptime  # Load should be < 1.0

# Monitor temperature (optional, keep open)
watch -n 1 sensors
```

### 2. Run Benchmarks

#### All Arc Benchmarks

```bash
cargo bench --bench arc_ops
```

**Expected duration:** ~5-10 minutes

#### Specific Benchmark Groups

```bash
# Insert operations only
cargo bench --bench arc_ops -- arc_insert

# Get operations only
cargo bench --bench arc_ops -- arc_get

# vs DashMap comparison
cargo bench --bench arc_ops -- arc_vs_dashmap

# Concurrent tests
cargo bench --bench arc_ops -- arc_concurrent

# Mixed workload
cargo bench --bench arc_ops -- arc_mixed
```

### 3. Baseline Comparison

```bash
# Save first run as baseline
cargo bench --bench arc_ops -- --save-baseline v0.3.0-initial

# After optimizations, compare
cargo bench --bench arc_ops -- --baseline v0.3.0-initial
```

### 4. Results Location

```
target/criterion/
├── arc_insert/
│   ├── arc_string/
│   ├── arc_vec_small_4b/
│   └── report/
│       └── index.html
├── arc_get/
├── arc_vs_dashmap_insert/
└── report/
    └── index.html  ← Open this for full report
```

**View results:**
```bash
firefox target/criterion/report/index.html
```

---

## Interpreting Results

### Understanding Criterion Output

```
arc_insert/arc_string
                        time:   [245.32 ns 247.89 ns 250.67 ns]
                                 ↑         ↑         ↑
                                 Lower     Mean      Upper (95% CI)

                        change: [-2.3% +0.5% +3.4%] (p = 0.45 > 0.05)
                                 ↑                   ↑
                                 % change vs prev    Statistical significance

                        thrpt:  [3.989 Melem/s 4.033 Melem/s 4.078 Melem/s]
```

### Target Validation

| Benchmark | Target | Good | Acceptable | Needs Work |
|-----------|--------|------|------------|------------|
| arc_insert | <500ns | <300ns | 300-500ns | >500ns |
| arc_get | <100ns | <80ns | 80-100ns | >100ns |
| arc_update | <1μs | <700ns | 700-1000ns | >1μs |
| arc_remove | <500ns | <300ns | 300-500ns | >500ns |

### DashMap Comparison (B15 Guidelines)

**Example output:**
```
arc_vs_dashmap_insert/atomic_capsule_map
                        time:   [237.54 ns 239.12 ns 240.83 ns]

arc_vs_dashmap_insert/dashmap
                        time:   [312.67 ns 315.42 ns 318.35 ns]
```

**Calculate speedup:**
```
Speedup = DashMap time / AtomicCapsuleMap time
        = 315.42 / 239.12
        = 1.319 (31.9% faster)
```

**Interpret:**
- 10-50% faster: **Typical** ✅ (Expected, document)
- 50-100% faster: **Good** 👍 (Verify, document)
- 100-200% (2x): **Exceptional** ⚠️ (Deep validation required)
- >200% (>2x): **Suspicious** 🚨 (Intensive validation, check methodology)

### Concurrent Scaling

**Expected patterns:**

| Threads | Ideal Speedup | Realistic | Quality |
|---------|---------------|-----------|---------|
| 1 | 1.0x | 1.0x | Baseline |
| 2 | 2.0x | 1.8-1.9x | Excellent |
| 4 | 4.0x | 3.2-3.6x | Good |
| 8 | 8.0x | 5.0-6.5x | Acceptable |

**AtomicCapsuleMap advantage should grow:**
- 1 thread: 10-30% faster than DashMap
- 4 threads: 30-70% faster than DashMap
- 8 threads: 50-100% faster than DashMap

---

## Quick Validation Checklist

After running benchmarks, check:

### ✅ Performance Targets

```bash
# Extract mean times from Criterion output
grep "time:" target/criterion/arc_insert/arc_string/base/estimates.json

# Or view in HTML report
```

- [ ] `arc_insert/arc_string`: <500ns
- [ ] `arc_get/arc_string/1000`: <100ns
- [ ] `arc_update/arc_string_replace`: <1μs
- [ ] `arc_remove/arc_string`: <500ns

### ✅ DashMap Comparison

- [ ] `arc_vs_dashmap_insert`: 10-50% faster (typical)
- [ ] `arc_vs_dashmap_get`: 10-50% faster (typical)
- [ ] No result >2x without deep validation

### ✅ Regression Detection

- [ ] Arc overhead vs u64: 50-100ns insert, 10-20ns get
- [ ] Variance <15% (check std dev in report)
- [ ] P99 < 2× P50 (stable tail latency)

### ✅ Concurrent Scaling

- [ ] 1→2 threads: ~1.8x speedup
- [ ] 2→4 threads: ~3.5x speedup
- [ ] 4→8 threads: ~6x speedup
- [ ] AtomicCapsuleMap gap widens with threads

---

## Troubleshooting

### High Variance (>15%)

**Symptoms:**
```
Std Dev: [45.3 ns]  (18.3% of mean)  ← Too high
```

**Solutions:**
```bash
# 1. Check CPU throttling
sensors | grep Core

# 2. Check system load
uptime

# 3. Kill background processes
systemctl stop packagekit
systemctl stop snapd

# 4. Pin to P-cores (if available)
taskset -c 0-5 cargo bench --bench arc_ops
```

### Slower Than Expected

**Check compiler optimizations:**
```bash
# Verify release mode
cargo bench --bench arc_ops --release  # Should be default

# Check RUSTFLAGS
echo $RUSTFLAGS  # Should include: -C target-cpu=native -C lto=fat

# Set if missing
export RUSTFLAGS="-C target-cpu=native -C lto=fat"
cargo clean
cargo bench --bench arc_ops
```

### DashMap Appears Faster

**Possible causes:**
1. Arc implementation not complete
2. Missing optimizations in Arc path
3. Unfair comparison (different workloads)

**Verify:**
```bash
# Compare same operations
cargo bench --bench arc_ops -- arc_vs_dashmap_insert
cargo bench --bench arc_ops -- arc_vs_dashmap_get

# Profile to find bottleneck
cargo build --release --bench arc_ops
perf record --call-graph dwarf target/release/deps/arc_ops-* --bench
perf report
```

### Concurrent Tests Fail

**Common issues:**
- Thread creation overhead dominates
- NUMA effects (cross-node latency)
- Memory bandwidth saturation

**Debug:**
```bash
# Check thread affinity
taskset -c 0-7 cargo bench --bench arc_ops -- arc_concurrent

# Monitor memory bandwidth
perf stat -e cycles,instructions,cache-references,cache-misses \
    cargo bench --bench arc_ops -- arc_concurrent/8
```

---

## Advanced Analysis

### Detailed Profiling

```bash
# Build benchmark binary
cargo build --release --bench arc_ops

# Find binary
BENCH_BIN=$(find target/release/deps -name 'arc_ops-*' -type f -executable | head -1)

# Profile with perf
perf record --call-graph dwarf $BENCH_BIN --bench --profile-time 10
perf report

# Cache analysis
perf stat -e cache-references,cache-misses,L1-dcache-loads,L1-dcache-load-misses \
    $BENCH_BIN --bench --profile-time 10
```

### Assembly Inspection

```bash
# Install cargo-asm
cargo install cargo-asm

# View insert assembly
cargo asm --release atomic_capsule_map::api::insert

# Look for:
# - Atomic instructions (lock cmpxchg, xadd)
# - Cache line operations
# - Unnecessary branches
```

### Flamegraph Generation

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bench arc_ops -- --bench --profile-time 30

# Open flamegraph.svg
firefox flamegraph.svg
```

---

## Benchmark Comparison Script

Save as `compare_results.sh`:

```bash
#!/bin/bash
# Extract key metrics from Criterion results

RESULTS_DIR="target/criterion"

echo "=== Arc<T> Benchmark Results ==="
echo

for bench in arc_insert arc_get arc_update arc_remove; do
    if [ -d "$RESULTS_DIR/$bench" ]; then
        echo "=== $bench ==="
        for subdir in "$RESULTS_DIR/$bench"/*; do
            if [ -d "$subdir" ] && [ -f "$subdir/base/estimates.json" ]; then
                name=$(basename "$subdir")
                mean=$(jq -r '.mean.point_estimate' "$subdir/base/estimates.json")
                mean_ns=$(printf "%.2f" $(echo "$mean / 1000000" | bc -l))
                echo "  $name: ${mean_ns}ns"
            fi
        done
        echo
    fi
done

echo "=== DashMap Comparison ==="
acm_insert=$(jq -r '.mean.point_estimate' "$RESULTS_DIR/arc_vs_dashmap_insert/atomic_capsule_map/base/estimates.json" 2>/dev/null)
dm_insert=$(jq -r '.mean.point_estimate' "$RESULTS_DIR/arc_vs_dashmap_insert/dashmap/base/estimates.json" 2>/dev/null)

if [ ! -z "$acm_insert" ] && [ ! -z "$dm_insert" ]; then
    speedup=$(echo "scale=3; $dm_insert / $acm_insert" | bc)
    pct=$(echo "scale=1; ($speedup - 1) * 100" | bc)
    echo "Insert: ${pct}% faster than DashMap (${speedup}x)"
fi

acm_get=$(jq -r '.mean.point_estimate' "$RESULTS_DIR/arc_vs_dashmap_get/atomic_capsule_map/base/estimates.json" 2>/dev/null)
dm_get=$(jq -r '.mean.point_estimate' "$RESULTS_DIR/arc_vs_dashmap_get/dashmap/base/estimates.json" 2>/dev/null)

if [ ! -z "$acm_get" ] && [ ! -z "$dm_get" ]; then
    speedup=$(echo "scale=3; $dm_get / $acm_get" | bc)
    pct=$(echo "scale=1; ($speedup - 1) * 100" | bc)
    echo "Get: ${pct}% faster than DashMap (${speedup}x)"
fi
```

**Usage:**
```bash
chmod +x compare_results.sh
./compare_results.sh
```

---

## Reporting Results

### Minimal Report Template

```markdown
# Arc<T> Benchmark Results

**Hardware:** Intel Ultra 7 155H (6P+8E cores, 24MB L3)
**Date:** 2025-XX-XX
**Rust:** 1.88.0-nightly

## Performance Targets

| Benchmark | Target | Actual | Status |
|-----------|--------|--------|--------|
| arc_insert | <500ns | XXXns | ✅/❌ |
| arc_get | <100ns | XXXns | ✅/❌ |
| arc_update | <1μs | XXXns | ✅/❌ |
| arc_remove | <500ns | XXXns | ✅/❌ |

## DashMap Comparison

| Operation | AtomicCapsuleMap | DashMap | Speedup |
|-----------|------------------|---------|---------|
| Insert | XXXns | XXXns | XX% faster |
| Get | XXXns | XXXns | XX% faster |

## Concurrent Scaling (8 threads)

| Threads | AtomicCapsuleMap | DashMap | Advantage |
|---------|------------------|---------|-----------|
| 1 | XXXns | XXXns | XX% |
| 4 | XXXns | XXXns | XX% |
| 8 | XXXns | XXXns | XX% |

## Conclusion

[Brief analysis of results vs targets and DashMap]
```

---

## Next Steps

After benchmarking:

1. **Document Results:** Fill in `ARC_BENCHMARK_RESULTS.md`
2. **Validate Targets:** All operations meet sub-microsecond goals?
3. **DashMap Analysis:** 10-50% improvement achieved?
4. **Optimize:** If targets not met, profile and improve
5. **Update README:** Add performance claims with evidence

**Remember:** Honest gains are 10-50% typical, 2x exceptional, 10x suspicious. Document methodology and be transparent about limitations.
