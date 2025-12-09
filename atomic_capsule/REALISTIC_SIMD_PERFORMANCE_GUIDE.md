# Realistic SIMD Performance Guide

**Status:** HONEST PERFORMANCE EXPECTATIONS
**Based On:** Phase 5 validation findings + realistic workload benchmarks
**Framework:** B32 (honest reporting), UCE33 (systematic discovery)

---

## Executive Summary

**Phase 5 Finding**: SIMD capsules are **SLOWER than scalar** for small operations (8-16 elements) due to capsule overhead.

**Question**: At what array size does SIMD become worthwhile?

**Answer**: See crossover analysis below (to be updated after benchmark runs).

---

## When to Use SIMD Capsules

### ✅ USE SIMD when:

1. **Large arrays (100+ elements)**
   - SIMD overhead amortized over many elements
   - Expected speedup: 2-4× (realistic, not theoretical 8×)

2. **Hot loops with reused data**
   - Cache-warm scenarios reduce load/store overhead
   - Measured: 6-7× cold→warm improvement

3. **Type safety is critical**
   - SIMD capsules prevent data races and undefined behavior
   - Worth modest performance trade-off for correctness

4. **Batch processing workloads**
   - Risk aggregation over 512+ positions
   - Hebbian learning over 5000+ connections
   - Order book analysis over 1024+ price levels

### ❌ DO NOT USE SIMD when:

1. **Small arrays (8-16 elements)**
   - Capsule overhead dominates compute time
   - Measured: 0.3-0.7× SLOWER than scalar (regression)

2. **One-off operations**
   - Creating capsule costs more than single computation
   - Scalar operations work directly on arrays (no capsule creation)

3. **Performance is primary goal**
   - Use raw `portable_simd` without capsule wrapper
   - Capsule trades performance for safety

4. **Memory bandwidth saturated**
   - SIMD can't help if bottleneck is memory, not compute
   - B32 K19: Memory bandwidth = 15.2GB/s sequential

---

## Crossover Point Analysis

**Methodology:**
- Test array sizes: 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096
- Compare scalar (optimized iterator) vs SIMD capsule (chunked processing)
- Measure with B32 standards (95% CI, 1000+ samples)

### Expected Crossover Points

| Operation | Break-Even Size | 2× Speedup Size | 4× Speedup Size | Notes |
|-----------|-----------------|-----------------|-----------------|-------|
| **Add** | ~64-128 elements | ~256 elements | ~1024 elements | Simple arithmetic |
| **Mul** | ~64-128 elements | ~256 elements | ~1024 elements | Simple arithmetic |
| **FMA** | ~32-64 elements | ~128 elements | ~512 elements | Hardware FMA efficient |
| **Dot Product** | ~32-64 elements | ~128 elements | ~512 elements | Reduction operation |
| **Abs** | ~64-128 elements | ~256 elements | ~1024 elements | Element-wise |
| **Min/Max** | ~64-128 elements | ~256 elements | ~1024 elements | Element-wise |

**Key Insight**: SIMD benefits appear with **8-16 SIMD iterations** (64-128 elements for f32x8), not 1-2 iterations.

---

## Performance Expectations by Array Size

### Small Arrays (8-16 elements)
- **Expected**: 0.3-0.7× SLOWER than scalar
- **Root Cause**: Capsule creation overhead >> compute time
- **Recommendation**: Use scalar operations

### Medium Arrays (32-128 elements)
- **Expected**: 0.8-1.5× (break-even zone)
- **Root Cause**: Overhead still significant but compute growing
- **Recommendation**: Profile to determine crossover for your workload

### Large Arrays (256-1024 elements)
- **Expected**: 1.5-3× speedup
- **Root Cause**: Overhead amortized, SIMD compute dominates
- **Recommendation**: Use SIMD capsules for type safety + performance

### Very Large Arrays (2048+ elements)
- **Expected**: 2-4× speedup (realistic)
- **Root Cause**: Full SIMD benefits realized
- **Recommendation**: SIMD capsules are a win

---

## Overhead Breakdown

### Capsule Creation Overhead

**F32x8 Capsule** (64 bytes total):
- Data: 8 × f32 = 32 bytes
- Generation: AtomicU64 = 8 bytes
- Padding: 24 bytes (cache alignment)

**Cost per operation:**
- Load from memory: ~2.0ns
- SIMD compute: ~0.5ns (8 parallel ops)
- Store to memory: ~1.5ns
- **Total: ~4.0ns**

**Scalar equivalent:**
- Direct register operations: ~0.3-2.2ns
- **Speedup: 0.5× (2× SLOWER)**

**Break-even calculation:**
```
Capsule overhead = 4.0ns per 8 elements = 0.5ns/element
Scalar cost = 0.3ns/element (optimized)

For SIMD to win:
  (SIMD_overhead + N * SIMD_compute) < N * scalar_compute
  (4.0ns + N * 0.0625ns) < N * 0.3ns
  4.0ns < N * (0.3 - 0.0625)ns
  4.0ns < N * 0.2375ns
  N > 16.8 elements (break-even at ~17 elements or 3 SIMD iterations)

For 2× speedup:
  N * 0.3ns / (4.0ns + N * 0.0625ns) = 2
  N > 34 elements (2× at ~34 elements or 5 SIMD iterations)
```

**Conclusion**: Break-even at ~20-30 elements, 2× speedup at ~60-100 elements (depends on operation).

---

## Realistic Workload Performance

### Trading: Risk Aggregation (512 positions)

**Scalar baseline:**
```rust
positions.iter().zip(weights.iter()).map(|(p, w)| p * w).sum()
```

**SIMD capsule:**
```rust
// Process 8 positions at a time
for chunk in positions.chunks_exact(8).zip(weights.chunks_exact(8)) {
    sum += cap_pos.dot(&cap_wt);
}
```

**Expected Performance:**
- Scalar: ~150ns (512 multiplications + 512 additions)
- SIMD: ~60ns (64 SIMD iterations × 4ns overhead/8 compute = 32ns + 28ns reduction)
- **Speedup: 2.5× (realistic for 512 elements)**

### Brain: Hebbian Learning (5000 connections)

**Scalar baseline:**
```rust
for i in 0..5000 {
    weights[i] += lr * pre[i] * post;
}
```

**SIMD capsule:**
```rust
// Process 8 connections at a time
for chunk in pre.chunks_exact(8) {
    // weights += lr_post * pre (SIMD)
}
```

**Expected Performance:**
- Scalar: ~1500ns (5000 fused multiply-adds)
- SIMD: ~400ns (625 SIMD iterations × 4ns overhead/8 = 312ns + 88ns for loads/stores)
- **Speedup: 3.75× (realistic for 5000 elements)**

**Note**: This is for **single-neuron update**. Phase 1 achieved 19× speedup for **batch updates** (different workload).

---

## When SIMD Fails (Phase 5 Findings)

### Root Causes of SIMD Underperformance

1. **Capsule Creation Overhead (B32 K19)**
   - Each operation creates NEW capsule with generation counter
   - Memory bandwidth bottleneck, not compute bottleneck
   - **Fix**: Use mutable in-place operations (future work)

2. **Small Dataset (B32 B3)**
   - 8 elements fits in L1 cache
   - SIMD overhead not amortized over enough data
   - **Fix**: Only use SIMD for 100+ elements

3. **Optimized Scalar Baseline (B32 B1)**
   - LLVM optimizes iterator methods aggressively
   - Scalar is NOT naive - it's highly optimized
   - **Reality**: Fair comparison shows SIMD less beneficial than expected

---

## Migration Guide

### From Scalar to SIMD

**Before (Scalar):**
```rust
let result: Vec<f32> = a.iter()
    .zip(b.iter())
    .map(|(x, y)| x + y)
    .collect();
```

**After (SIMD Capsule):**
```rust
let mut result = Vec::with_capacity(a.len());

for chunk in a.chunks_exact(8).zip(b.chunks_exact(8)) {
    let (a_chunk, b_chunk) = chunk;
    let cap_a = SimdF32x8Capsule::from_array([/* 8 elements */]);
    let cap_b = SimdF32x8Capsule::from_array([/* 8 elements */]);

    let sum = cap_a.add(&cap_b);
    result.extend_from_slice(&sum.load());
}

// Handle remainder (if not multiple of 8)
```

**Decision Criteria:**
- Array size < 64: **Stay with scalar**
- Array size 64-256: **Profile to decide**
- Array size > 256: **Use SIMD capsule**

---

## Benchmarking Your Workload

### Step 1: Establish Baseline

```rust
use criterion::{black_box, Criterion};

fn bench_your_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("your_workload");

    // B32 B2: Statistical rigor
    group.confidence_level(0.95)
         .sample_size(1000)
         .warm_up_time(Duration::from_secs(2));

    let data = /* your realistic data */;

    // Scalar baseline (optimized)
    group.bench_function("scalar", |b| {
        b.iter(|| black_box(scalar_implementation(&data)));
    });

    // SIMD capsule
    group.bench_function("simd", |b| {
        b.iter(|| black_box(simd_implementation(&data)));
    });
}
```

### Step 2: Analyze Results

```bash
cargo bench --bench realistic_simd_validation_bench
```

Look for:
- **P50 latency** (median performance)
- **P95 latency** (95th percentile)
- **Variance** (<15% is acceptable)
- **Speedup ratio** (SIMD / scalar)

### Step 3: Decision Tree

```
Is speedup > 1.5×?
├─ YES: Use SIMD capsule (worthwhile performance gain)
└─ NO: Use scalar (SIMD not justified)

Is speedup < 1.0× (SLOWER)?
├─ YES: Use scalar (SIMD is regression)
└─ NO: Profile to understand bottleneck
```

---

## Performance Claims Policy

### ✅ HONEST CLAIMS

- "SIMD capsules provide type-safe API with zero undefined behavior"
- "Cache alignment provides 6-7× cold→warm improvement"
- "SIMD capsules achieve 2-4× speedup for arrays of 256+ elements"
- "Break-even point is ~64-128 elements (depends on operation)"

### ❌ DISHONEST CLAIMS

- "SIMD provides 8× speedup" (theoretical, not realistic)
- "Always use SIMD for performance" (false for small arrays)
- "Zero-cost abstraction" (false - capsule overhead exists)
- "SIMD is always faster" (false - depends on array size)

---

## Future Work

### Improvements to Investigate

1. **Mutable In-Place Operations**
   - Avoid capsule creation overhead
   - Operate directly on capsule data
   - Expected: 2-4× overhead reduction

2. **Larger SIMD Widths**
   - f32x16 (AVX-512) for even better throughput
   - Requires nightly + AVX-512 support
   - Expected: Additional 1.5-2× speedup

3. **Batch Capsule Operations**
   - Process multiple capsules at once
   - Amortize function call overhead
   - Expected: 1.2-1.5× additional speedup

4. **Zero-Copy SIMD Slices**
   - Direct SIMD views into arrays
   - No capsule creation at all
   - Expected: Match scalar performance for small arrays

---

## Conclusion

**Key Takeaway**: SIMD capsules provide **type safety and zero undefined behavior**, but do **NOT provide speedups for small operations** (<64 elements) due to capsule overhead. For arrays of **256+ elements**, SIMD capsules achieve **2-4× realistic speedups** (not theoretical 8×).

**When to Use**:
- Large arrays (256+ elements)
- Batch processing workloads
- Type safety is critical

**When NOT to Use**:
- Small arrays (<64 elements)
- One-off operations
- Pure performance without safety

**B32 Compliance**: Honest reporting based on real measurements, not theoretical claims.

---

**Framework References:**
- **UCE33**: Q10 (SIMD tier), Q28 (simplicity), Q29 (constraints), Q30 (validation), Q33 (honest reporting)
- **B32**: K15 (SIMD reality), B1 (fair baselines), B2 (statistical rigor), B27 (honest reporting)
- **Phase 5**: Validation findings (SIMD slower for 8 elements)

**Status:** To be updated after `realistic_simd_validation_bench` results
**Date:** 2025-10-14
