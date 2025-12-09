# Arc<T> Benchmark Design & Analysis
## B32 Framework Compliant Performance Validation

### Executive Summary

This document describes the comprehensive Arc<T> benchmark suite for atomic_capsule_map v0.3.0, designed following the B32 Benchmark Framework and The Atomic Capsule performance targets.

**Key Objectives:**
1. Validate Arc<T> operations meet sub-microsecond latency targets
2. Compare fairly against DashMap (industry-standard baseline)
3. Measure Arc refcount overhead vs primitive Copy types
4. Verify no performance regression for existing primitives
5. Test concurrent access patterns (1-8 threads)

---

## B32 Framework Compliance

### B1: Fair Baseline Selection ✅
- **Baseline**: DashMap 6.1 (industry-standard concurrent hashmap)
- **Why Fair**: DashMap is optimized, widely used, actively maintained
- **Not Strawman**: Comparing against best-in-class, not `std::sync::Mutex`

### B2: Measurement Methodology ✅
- **Tool**: Criterion.rs for statistical rigor
- **Confidence Intervals**: 95% CI required
- **Sample Size**: 1000+ iterations per benchmark
- **Warmup Period**: 3 seconds (B8: Cache warming)
- **Multiple Runs**: Criterion automatically handles this

### B3: Realistic Workloads ✅
- **Arc<String>**: Most common real-world use case
- **Arc<Vec<u8>>**: Various sizes (4B, 64B, 1KB)
- **Mixed Operations**: 70% reads, 20% updates, 10% inserts
- **Pre-populated Maps**: Test realistic load scenarios

### B4: Contention Scenarios ✅
- **1 thread**: Uncontended baseline
- **2 threads**: Light contention
- **4 threads**: Moderate contention
- **8 threads**: Heavy contention (approaching K12 sweet spot)

### B5: Reporting Standards ✅
Will report:
- P50, P95, P99 percentiles (Criterion provides these)
- Mean ± standard deviation
- Hardware specifications
- Compiler version and flags
- Thermal conditions

### B7: Memory Allocation Patterns ✅
- Pre-allocate Arc values before measurement
- Reuse Arc values where appropriate
- Measure allocation overhead separately

### B8: Cache Warming Strategy ✅
- 3-second warmup period per benchmark
- Pre-populate maps before read benchmarks
- Discard first iterations automatically (Criterion)

### B15: Realistic Performance Expectations ✅
**Expected improvements over DashMap:**
- **Typical**: 10-50% faster for uncontended operations
- **Exceptional**: 2x faster under heavy contention (lockfree advantage)
- **Suspicious**: Any claim of 10x+ would require extraordinary validation

---

## Performance Targets (from The Atomic Capsule)

### Arc Operation Targets

| Operation | Target | Rationale |
|-----------|--------|-----------|
| Arc insert | <500ns | Refcount increment (5ns) + atomic publish (15ns CAS) + overhead |
| Arc get | <100ns | Lockfree read (5ns) + refcount increment (5ns) + overhead |
| Arc update | <1μs | CAS (15ns) + old Arc drop (5ns) + new Arc store + overhead |
| Arc remove | <500ns | Atomic remove (15ns) + Arc drop (5ns) + overhead |

### Hardware Reality (K1-K9 from B32)

**Intel Ultra 7 155H Baseline:**
- AtomicU64 CAS: 10-15ns actual
- L1 Cache: 1ns latency
- L2 Cache: 3ns latency
- L3 Cache: 12ns latency
- Arc clone: ~5ns (atomic fetch_add on refcount)
- Arc drop: ~5ns (atomic fetch_sub + conditional dealloc)

**Cache Line Awareness:**
- Atomic capsules: 64-byte aligned
- Prevents false sharing
- Critical for concurrent performance

---

## Benchmark Structure

### 1. Arc Insert Benchmarks (`bench_arc_insert`)

Tests Arc<T> insertion performance with various payload sizes:

```rust
- arc_string:          Arc<String> (heap-allocated, realistic)
- arc_vec_small_4b:    Arc<Vec<u8>> with 4-byte payload
- arc_vec_medium_64b:  Arc<Vec<u8>> with 64-byte payload (cache line)
- arc_vec_large_1kb:   Arc<Vec<u8>> with 1KB payload
```

**What we measure:**
- Arc allocation overhead
- Refcount initialization
- Atomic capsule publication
- Cache effects with different payload sizes

**Expected results:**
- All operations <500ns
- Small payloads closer to 100-200ns
- Large payloads may approach 400-500ns due to allocation

### 2. Arc Get Benchmarks (`bench_arc_get`)

Tests lockfree read + Arc clone performance:

```rust
- arc_string (sizes: 100, 1000, 10000)
- arc_vec_64b (sizes: 100, 1000, 10000)
```

**What we measure:**
- Lockfree read latency
- Arc refcount increment cost
- Cache hit/miss rates at different map sizes

**Expected results:**
- <100ns per get operation
- Performance degradation as map size exceeds L3 cache (24MB)

### 3. Arc Update Benchmarks (`bench_arc_update`)

Tests in-place Arc replacement:

```rust
- arc_string_replace:  Replace existing Arc<String>
- arc_vec_replace:     Replace existing Arc<Vec<u8>>
```

**What we measure:**
- CAS operation latency
- Old Arc drop overhead
- New Arc store overhead

**Expected results:**
- <1μs per update
- Dominated by Arc allocation, not atomic operation

### 4. Arc Remove Benchmarks (`bench_arc_remove`)

Tests Arc removal and deallocation:

```rust
- arc_string:  Remove Arc<String>
```

**What we measure:**
- Atomic removal operation
- Arc drop and deallocation

**Expected results:**
- <500ns per remove
- May vary with Arc refcount complexity

### 5. Arc vs DashMap Comparison (B1: Fair Baseline)

Direct head-to-head comparison:

```rust
bench_arc_vs_dashmap_insert:
  - atomic_capsule_map
  - dashmap

bench_arc_vs_dashmap_get:
  - atomic_capsule_map
  - dashmap
```

**What we measure:**
- Relative performance vs industry standard
- Lockfree advantage quantification

**Expected results (B15):**
- Insert: 10-50% faster (typical)
- Get: 10-50% faster uncontended, up to 2x under contention

### 6. Arc Refcount Overhead Analysis

Compares Arc<T> vs primitive Copy types:

```rust
- baseline_u64_insert:  Plain u64 (no refcount)
- arc_u64_insert:       Arc<u64> (refcount overhead)
- baseline_u64_get:     Plain u64 get
- arc_u64_get:          Arc<u64> get (clone overhead)
```

**What we measure:**
- Pure Arc overhead isolated from payload complexity
- Refcount atomic operation cost

**Expected results:**
- Arc insert: ~50-100ns overhead vs u64
- Arc get: ~10-20ns overhead vs u64 (single refcount increment)

### 7. Concurrent Arc Operations (B4: Contention Scenarios)

Tests 1, 2, 4, 8 threads with realistic 70% read / 30% write mix:

```rust
For each thread count:
  - atomic_capsule_map
  - dashmap
```

**What we measure:**
- Lockfree scaling efficiency
- Contention impact on latency
- Thread coordination overhead

**Expected results:**
- Linear scaling 1→2 threads (90%+)
- Sublinear scaling 2→4 threads (70-80%)
- Diminishing returns 4→8 threads (50-70%)
- AtomicCapsuleMap advantage increases with contention

### 8. Realistic Mixed Workload

70% reads, 20% updates, 10% inserts (production-like):

```rust
- atomic_capsule_map
- dashmap
```

**What we measure:**
- Real-world performance simulation
- Cache effects with mixed operations
- Overall system throughput

**Expected results:**
- 10-50% improvement over DashMap (typical case)
- Smaller improvement than pure read benchmarks due to write overhead

---

## Benchmark Execution

### Running Benchmarks

```bash
# Run all Arc benchmarks
cargo bench --bench arc_ops

# Run specific benchmark group
cargo bench --bench arc_ops -- arc_insert
cargo bench --bench arc_ops -- arc_vs_dashmap

# Generate detailed report with plots
cargo bench --bench arc_ops -- --save-baseline arc_v0.3.0

# Compare against baseline
cargo bench --bench arc_ops -- --baseline arc_v0.3.0
```

### Expected Output Format

```
arc_insert/arc_string
                        time:   [245.32 ns 247.89 ns 250.67 ns]
                        change: [-2.3% +0.5% +3.4%] (p = 0.45 > 0.05)
                        No change in performance detected.

arc_get/arc_string/1000
                        time:   [67.42 ns 68.15 ns 68.91 ns]
                        thrpt:  [14.514 Melem/s 14.674 Melem/s 14.832 Melem/s]

arc_vs_dashmap_insert/atomic_capsule_map
                        time:   [237.54 ns 239.12 ns 240.83 ns]

arc_vs_dashmap_insert/dashmap
                        time:   [312.67 ns 315.42 ns 318.35 ns]

Comparison: atomic_capsule_map is 31.9% faster than dashmap (p < 0.001)
```

---

## Analysis Guidelines

### 1. Target Achievement Validation

Check each benchmark group against targets:

```
✅ Arc insert: Mean <500ns?
✅ Arc get: Mean <100ns?
✅ Arc update: Mean <1μs?
✅ Arc remove: Mean <500ns?
```

### 2. DashMap Comparison (B15)

**Interpret results realistically:**

| Speedup | Classification | Action |
|---------|---------------|--------|
| 10-50% | Typical | Document as expected improvement |
| 50-100% | Good | Validate with multiple runs, document |
| 100-200% (2x) | Exceptional | Deep analysis, verify methodology |
| >200% (>2x) | Suspicious | Intensive validation required |

**Red flags:**
- >2x improvement without clear algorithmic advantage
- Inconsistent results across runs
- Large variance (>15% standard deviation)
- Performance cliffs at specific thread counts

### 3. Contention Scaling Analysis

**Expected patterns:**
- 1→2 threads: ~1.9x throughput (near-linear)
- 2→4 threads: ~3.5x throughput (sublinear)
- 4→8 threads: ~6x throughput (diminishing)

**AtomicCapsuleMap advantage should increase with threads:**
- 1 thread: 10-30% faster
- 4 threads: 30-70% faster
- 8 threads: 50-100% faster (lockfree shines under contention)

### 4. Regression Detection

Compare against primitive benchmarks:

```bash
# Run both primitive and Arc benchmarks
cargo bench --bench basic_ops -- u64
cargo bench --bench arc_ops -- arc_u64

# Compare: Arc overhead should be <100ns
```

**Acceptable overhead:**
- Insert: 50-100ns (Arc allocation + refcount init)
- Get: 10-20ns (Arc clone = atomic increment)

---

## Hardware Validation

### Test Configuration

**Required documentation:**
```
Hardware: Intel Ultra 7 155H (6P+8E cores, 24MB L3)
RAM: 64GB DDR5-5600
OS: Linux 6.14.0-32-generic
Rust: 1.88.0-nightly (2025-01-XX)
Flags: RUSTFLAGS="-C target-cpu=native -C lto=fat"
Cooling: Active (65W sustained, no throttling)
```

### Pre-benchmark Checklist

- [ ] Disable CPU frequency scaling: `sudo cpupower frequency-set -g performance`
- [ ] Close background applications
- [ ] Monitor thermal throttling: `sensors` or `turbostat`
- [ ] Verify no swap usage: `free -h`
- [ ] Check system load: `uptime` (load <1.0 ideal)

### Post-benchmark Validation

- [ ] Check Criterion HTML reports in `target/criterion/`
- [ ] Verify confidence intervals are tight (<5% variance)
- [ ] Review outlier analysis in reports
- [ ] Compare P50 vs P99 (should be <2x difference for stable performance)
- [ ] Check for thermal throttling in system logs

---

## Statistical Rigor (B2)

### Criterion Configuration

All benchmarks use:
```rust
group.confidence_level(0.95)      // 95% CI required
     .sample_size(1000)           // 1000+ iterations
     .warm_up_time(Duration::from_secs(3));  // Cache warming
```

### Interpreting Results

**Criterion output includes:**
- Mean: Average latency
- Std Dev: Variability (should be <10% of mean)
- Median: P50 latency
- MAD: Median Absolute Deviation (robust variance measure)
- Outliers: Classified as mild/severe

**Quality criteria:**
- Std Dev <10% of mean: Excellent
- Std Dev 10-15% of mean: Acceptable
- Std Dev >15% of mean: Investigate (thermal? cache? contention?)

**Statistical significance:**
- p < 0.05: Significant difference detected
- p ≥ 0.05: No significant change (noise)

---

## Common Issues & Debugging

### Issue: High Variance (>15%)

**Possible causes:**
- Thermal throttling
- Background processes
- NUMA effects (cross-node access)
- Insufficient warmup

**Solutions:**
- Monitor CPU temperature during run
- Use `taskset` to pin to P-cores
- Increase warmup time
- Close unnecessary applications

### Issue: Unexpectedly Slow Performance

**Check:**
1. Compiler optimizations: `--release` flag?
2. LTO enabled: `RUSTFLAGS="-C lto=fat"`?
3. Target CPU: `RUSTFLAGS="-C target-cpu=native"`?
4. Debug assertions: Disabled in release?

### Issue: Results Don't Match Targets

**Analysis steps:**
1. Check hardware specs match baseline
2. Verify no background load
3. Review Criterion detailed report
4. Examine outliers (thermal/contention spikes?)
5. Compare with DashMap (relative vs absolute performance)

### Issue: Concurrent Benchmarks Show Regression

**Investigate:**
- Thread pinning (use `core_affinity` for P-cores)
- False sharing (check alignment with `perf c2c`)
- Lock contention (use `perf lock`)
- Memory bandwidth saturation (>15GB/s?)

---

## Next Steps After Benchmarking

### 1. Results Documentation

Create `ARC_BENCHMARK_RESULTS.md` with:
- Summary table (all benchmarks vs targets)
- DashMap comparison analysis
- Contention scaling graphs
- Hardware configuration
- Statistical significance notes

### 2. Performance Validation

- [ ] All operations meet targets?
- [ ] DashMap comparison shows 10-50% improvement?
- [ ] No regression for primitives?
- [ ] Concurrent scaling efficient (<12 threads)?

### 3. Optimization Opportunities

If targets not met:
- Profile with `perf` to find hotspots
- Check assembly output: `cargo asm`
- Analyze cache misses: `perf stat -e cache-misses`
- Consider SIMD for batch operations

### 4. Publication Readiness

Before v0.3.0 release:
- [ ] Benchmark results documented
- [ ] B32 compliance verified
- [ ] DashMap comparison fair and honest
- [ ] README updated with performance claims
- [ ] Changelog includes benchmark validation

---

## Conclusion

This benchmark suite provides comprehensive validation of Arc<T> support following the B32 Framework:

**✅ Fair baselines** (DashMap, not strawman)
**✅ Statistical rigor** (95% CI, 1000+ samples)
**✅ Realistic workloads** (Arc<String>, mixed operations)
**✅ Contention testing** (1-8 threads)
**✅ Honest expectations** (10-50% typical, 2x exceptional)

Run benchmarks, analyze results, validate targets, document findings.

**Expected outcome:** Arc<T> support meets sub-microsecond targets and shows 10-50% improvement over DashMap under typical workloads, with larger advantages under contention.
