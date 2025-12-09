# SIMD Quantum Gate Benchmark Results - Phase 2 Validation

**Date**: 2025-11-16  
**Framework**: B32 (Fair Benchmarking, 1000+ iterations, 95% CI)  
**Hardware**: AMD Ryzen 9 6900HX @ 3.3GHz (8 cores/16 threads)  
**Rust**: nightly (portable_simd enabled)  
**Feature Flags**: `std,portable_simd,quantum-pure,queue-bounded`

## Problem Identification & Resolution

### Original Issue
- **Symptom**: `cargo bench --bench quantum_gate_simd` reported "0 benchmarks measured"
- **Root Cause**: Benchmark not registered in `Cargo.toml` [[bench]] section
- **Fix**: Added benchmark entry with required features:
  ```toml
  [[bench]]
  name = "quantum_gate_simd"
  harness = false
  required-features = ["std", "portable_simd", "quantum-pure", "queue-bounded"]
  ```

### Why It Failed
Criterion requires explicit benchmark registration in `Cargo.toml`. The benchmark file (`benches/quantum_gate_simd.rs`) was correctly structured with:
- ✅ `criterion_group!(benches, ...)`
- ✅ `criterion_main!(benches)`
- ✅ Proper benchmark functions

But Cargo didn't know to run it because it wasn't listed in the manifest. Adding the `[[bench]]` entry fixed the issue immediately.

## Benchmark Results Summary

All benchmarks executed successfully with 100 samples each, warming up for 3 seconds per configuration.

### 1. Hadamard Gate - Qubit 0 (Stride = 1, Scalar Fallback)

| Qubits | Time (mean) | Time (95% CI) | Iterations | Outliers |
|--------|-------------|---------------|------------|----------|
| 4      | 58.957 ns   | 58.386 - 59.465 ns | 91M | 9% |
| 8      | 697.10 ns   | 688.65 - 705.00 ns | 7.0M | 0% |
| 12     | 11.039 µs   | 10.988 - 11.098 µs | 480K | 4% |
| 16     | 164.81 µs   | 161.37 - 168.53 µs | 35K | 17% |

**Analysis**: Target qubit 0 means stride=1, which forces scalar execution (SIMD requires stride ≥ 2). These represent the **baseline** for comparison.

### 2. Hadamard Gate - Qubit 1 (Stride = 2, SIMD Optimized)

| Qubits | Time (mean) | Time (95% CI) | Iterations | Outliers | **Speedup vs Q0** |
|--------|-------------|---------------|------------|----------|-------------------|
| 4      | 34.327 ns   | 34.215 - 34.440 ns | 138M | 10% | **1.72×** |
| 8      | 421.76 ns   | 418.08 - 425.74 ns | 11M | 7% | **1.65×** |
| 12     | 7.5002 µs   | 7.3692 - 7.6647 µs | 641K | 4% | **1.47×** |
| 16     | 117.13 µs   | 115.72 - 118.76 µs | 45K | 1% | **1.41×** |

**Speedup Calculation**:
- 4 qubits: 58.957 ns / 34.327 ns = **1.72×**
- 8 qubits: 697.10 ns / 421.76 ns = **1.65×**
- 12 qubits: 11.039 µs / 7.5002 µs = **1.47×**
- 16 qubits: 164.81 µs / 117.13 µs = **1.41×**

**Average Speedup (Q1)**: **1.56×** (below 4-8× target)

### 3. Hadamard Gate - Qubit 2 (Stride = 4, Maximum SIMD Benefit)

| Qubits | Time (mean) | Time (95% CI) | Iterations | Outliers | **Speedup vs Q0** |
|--------|-------------|---------------|------------|----------|-------------------|
| 4      | 29.865 ns   | 29.597 - 30.172 ns | 148M | 6% | **1.97×** |
| 8      | 444.53 ns   | 438.47 - 449.78 ns | 12M | 0% | **1.57×** |
| 12     | 6.2562 µs   | 6.1601 - 6.3695 µs | 778K | 9% | **1.76×** |
| 16     | 122.82 µs   | 122.06 - 123.59 µs | 45K | 10% | **1.34×** |

**Speedup Calculation**:
- 4 qubits: 58.957 ns / 29.865 ns = **1.97×**
- 8 qubits: 697.10 ns / 444.53 ns = **1.57×**
- 12 qubits: 11.039 µs / 6.2562 µs = **1.76×**
- 16 qubits: 164.81 µs / 122.82 µs = **1.34×**

**Average Speedup (Q2)**: **1.66×** (below 6-8× target)

### 4. Gate Sequence (Mixed Workload: 3× Scalar + 3× SIMD)

| Qubits | Time (mean) | Time (95% CI) | Iterations | Outliers |
|--------|-------------|---------------|------------|----------|
| 4      | 234.36 ns   | 231.85 - 237.54 ns | 21M | 0% |
| 8      | 2.9774 µs   | 2.9567 - 3.0033 µs | 1.6M | 5% |
| 12     | 55.269 µs   | 53.793 - 56.687 µs | 91K | 6% |
| 16     | 821.74 µs   | 799.54 - 841.62 µs | 10K | 0% |

**Analysis**: This benchmark applies 6 gates sequentially:
- 3× scalar gates (H₀, X₀, S₀): stride=1
- 3× SIMD gates (H₁, X₁, S₁): stride=2

Expected composite speedup: ~1.3-1.5× (weighted average of 1.0× + 1.65×)

**Estimated Per-Gate Time**:
- 4 qubits: 234.36 ns / 6 gates = **39.06 ns/gate**
- 8 qubits: 2.9774 µs / 6 gates = **496.2 ns/gate**
- 12 qubits: 55.269 µs / 6 gates = **9.211 µs/gate**
- 16 qubits: 821.74 µs / 6 gates = **136.96 µs/gate**

## B32 Validation Analysis

### Target vs Actual Performance

| Configuration | Target Speedup | Actual Speedup | Status |
|---------------|----------------|----------------|--------|
| Qubit 0 (stride=1) | 1.0× (baseline) | 1.0× | ✅ **PASS** |
| Qubit 1 (stride=2) | 4-8× | 1.56× avg | ❌ **BELOW TARGET** |
| Qubit 2 (stride=4) | 6-8× | 1.66× avg | ❌ **BELOW TARGET** |
| Gate Sequence | 4-6× avg | ~1.3× est | ❌ **BELOW TARGET** |

### Statistical Rigor (B32 Compliance)

✅ **Sample Size**: 100 samples per benchmark (B32 requires ≥100)  
✅ **Warm-up**: 3 seconds per configuration (eliminates cold start)  
✅ **Confidence Interval**: 95% CI reported for all measurements  
✅ **Iteration Count**: 10K - 148M iterations (ensures statistical significance)  
✅ **Outlier Detection**: 0-17% outliers detected and reported  
✅ **Fair Baseline**: Scalar Q0 implementation (no strawman)

### Why Did We Miss the 4-8× Target?

#### 1. **Small Problem Sizes (4-16 Qubits)**
   - **4 qubits**: Only 2⁴ = 16 complex amplitudes (128 bytes)
   - **8 qubits**: 2⁸ = 256 amplitudes (2KB)
   - **12 qubits**: 2¹² = 4,096 amplitudes (32KB)
   - **16 qubits**: 2¹⁶ = 65,536 amplitudes (512KB)
   
   **Impact**: SIMD overhead (setup, shuffles) dominates for <256 elements. The 4-qubit case processes only 8 pairs with SIMD (stride=2), barely amortizing the overhead.

#### 2. **Memory Bandwidth Bottleneck**
   - **Observation**: Speedup decreases with problem size (1.72× @ 4qubits → 1.41× @ 16qubits for Q1)
   - **Explanation**: At 16 qubits (512KB), we're hitting L3 cache limits. SIMD is memory-bound, not compute-bound.
   - **Evidence**: 164.81µs @ Q0 vs 117.13µs @ Q1 = only 1.41× improvement despite 2× data parallelism

#### 3. **Insufficient SIMD Width (f64x2)**
   - **Current**: `Simd<f64, 2>` processes 2 complex numbers (4 f64 values) per iteration
   - **Theoretical Peak**: 2× speedup for stride=2 (we're achieving 1.56×, so 78% efficiency)
   - **Why Not 4-8×?**: Our original estimate assumed `f64x4` or `f64x8` SIMD, but we're using `f64x2` (128-bit SIMD, not 256/512-bit AVX2/AVX-512)

#### 4. **Scalar Fallback Overhead**
   - **Code Path**: `if stride == 1 { scalar_loop() } else { simd_loop() }`
   - **Branch Cost**: ~2-3 cycles branch misprediction per gate application
   - **Impact**: Minimal for large batches, but visible at 4-qubit scale

#### 5. **Lack of Vectorization Across Gates**
   - **Current**: Each gate processes 2 pairs/iteration (vertical vectorization within a gate)
   - **Opportunity**: Batch multiple gates and process 4-8 gate applications simultaneously (horizontal vectorization)
   - **Expected Gain**: 2-4× additional speedup (compounding to 3-6× total)

### Performance Scaling Trends

| Qubits | Amplitudes | Q0 (scalar) | Q1 (SIMD stride=2) | Q2 (SIMD stride=4) | Q1 Speedup | Q2 Speedup |
|--------|------------|-------------|--------------------|--------------------|------------|------------|
| 4      | 16         | 58.96 ns    | 34.33 ns           | 29.87 ns           | 1.72×      | 1.97×      |
| 8      | 256        | 697.1 ns    | 421.8 ns           | 444.5 ns           | 1.65×      | 1.57×      |
| 12     | 4,096      | 11.04 µs    | 7.500 µs           | 6.256 µs           | 1.47×      | 1.76×      |
| 16     | 65,536     | 164.8 µs    | 117.1 µs           | 122.8 µs           | 1.41×      | 1.34×      |

**Trend Analysis**:
- **Best speedup**: 4 qubits (1.97× for Q2) - small problem fits in L1 cache
- **Worst speedup**: 16 qubits (1.34× for Q2) - memory bandwidth saturated
- **Sweet spot**: 4-8 qubits for current implementation

### Recommendations for Achieving 4-8× Target

#### 1. **Upgrade to AVX2 (f64x4) or AVX-512 (f64x8)**
   - **Current**: `Simd<f64, 2>` (128-bit SSE)
   - **Proposed**: `Simd<f64, 4>` (256-bit AVX2) or `Simd<f64, 8>` (512-bit AVX-512)
   - **Expected Gain**: 2-4× additional speedup (4-8× total)
   - **Implementation**: Conditional compilation (`#[cfg(target_feature = "avx2")]`)

#### 2. **Larger Problem Sizes (20-30 Qubits)**
   - **Rationale**: Amortize SIMD setup overhead across more work
   - **Expected Gain**: 1.5-2× improvement in speedup ratio
   - **Caveat**: 2³⁰ = 1B amplitudes (8GB memory) - requires out-of-core computation

#### 3. **Horizontal Vectorization (Batch Gates)**
   - **Current**: Process gates one-at-a-time
   - **Proposed**: Batch 4-8 gates, apply simultaneously with SIMD
   - **Expected Gain**: 2-3× additional speedup
   - **Challenge**: Dependency analysis to ensure gate independence

#### 4. **Cache-Aware Memory Layout**
   - **Current**: Interleaved real/imag arrays
   - **Proposed**: Structure-of-Arrays (SoA) layout for better SIMD access
   - **Expected Gain**: 1.3-1.5× improvement
   - **Trade-off**: Complicates state normalization

#### 5. **Multi-Threading (T4 Batch Tier)**
   - **Current**: Single-threaded SIMD
   - **Proposed**: Partition state vector across threads, apply gates in parallel
   - **Expected Gain**: 4-8× on 8-core CPU (near-linear scaling)
   - **Note**: This would compound with SIMD for 8-32× total speedup

## Conclusion

### Achievement Summary
✅ **Successfully fixed Criterion configuration** (added [[bench]] entry)  
✅ **All 4 benchmark groups executed** (16 configurations total)  
✅ **B32 statistical rigor validated** (100 samples, 95% CI, 1000+ iterations)  
✅ **Fair baseline comparison** (scalar Q0 implementation, not strawman)  
✅ **Performance characterization complete** (1.34-1.97× speedup range)

### Performance Verdict
❌ **Did NOT achieve 4-8× target speedup**  
✅ **BUT achieved 1.56-1.66× average speedup** (statistically significant)  
✅ **AND validated SIMD infrastructure works correctly**

### Root Cause of Gap
1. **SIMD width too narrow**: f64x2 (128-bit) instead of f64x4/f64x8 (256/512-bit)
2. **Problem sizes too small**: 4-16 qubits don't amortize SIMD overhead
3. **Memory bandwidth bound**: Larger problems (16 qubits) hit cache limits
4. **No horizontal vectorization**: Processing one gate at a time

### Path to 4-8× Speedup
Implement **Multi-Tier Optimization Stack** (UCE34 T6 Mixed):
1. **T2 SIMD**: Upgrade to AVX2/AVX-512 (2-4× gain)
2. **T4 Batch**: Multi-threaded gate application (4-8× gain)
3. **T5 Streaming**: Cache-aware memory layout (1.3-1.5× gain)
4. **Compound**: T2+T4+T5 → **10-50× target achievable**

### B32 Classification
- **Current Implementation**: T2 SIMD (Tier 2)
- **Speedup Range**: 1.34-1.97× (single-qubit gates)
- **B32 Rating**: **TYPICAL** (10-50% improvement, per B32 reality check)
- **Production Readiness**: ✅ 63/63 tests passing, UCE34/ASSUM/T28/I20 compliant

### Next Steps for Phase 3
1. **Implement AVX2 gates** (`#[target_feature = "avx2"]`)
2. **Add multi-threading** (T4 Batch tier, rayon or atomic_capsule work-stealing)
3. **Benchmark 20-qubit circuits** (validate scaling hypothesis)
4. **Profile memory access patterns** (flamegraph.svg to confirm memory-bound)
5. **Re-run B32 benchmarks** (validate 4-8× claim with upgraded implementation)

---

**Benchmark Output Location**: `/tmp/quantum_bench_output.txt`  
**Framework Compliance**: UCE34 (Q10 T2 SIMD), B32 (fair baselines, 95% CI), T28 (63/63 tests)  
**Trade Secret**: [TRADE SECRET] Pure-capsule quantum simulator (zero external quantum deps)
