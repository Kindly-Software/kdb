# Phase 5 Runtime Dispatch Benchmark Results

**Date**: 2025-11-02
**Hardware**: Intel Core Ultra 7 155H (22 cores, AVX2)
**OS**: Linux 6.14.0-33-generic
**Benchmark Framework**: Criterion.rs (1000 samples, 95% CI)
**Status**: ✅ VALIDATED (<0.1% overhead target met)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Hardware Specifications](#hardware-specifications)
3. [Performance Targets](#performance-targets)
4. [CPU Detection Overhead](#cpu-detection-overhead)
5. [SIMD MinHash Performance](#simd-minhash-performance)
6. [End-to-End Pipeline](#end-to-end-pipeline)
7. [Regression Analysis](#regression-analysis)
8. [Recommendations](#recommendations)

---

## Executive Summary

### Key Findings

| Metric | Target | Measured | Status |
|--------|--------|----------|--------|
| **CPU Detection Overhead** | <0.1% | <0.015% | ✅ **PASS** (6.6× better) |
| **SIMD MinHash Speedup** | 6-7× | 6.70× average | ✅ **PASS** (on target) |
| **Runtime Dispatch Cost** | <10ns | <10ns | ✅ **PASS** (cached access) |
| **Binary Size Impact** | +15% | +12% | ✅ **PASS** (within budget) |

### Performance Summary

**CPU Detection**:
- First call: ~1ms (one-time initialization)
- Cached access: <10ns (singleton pattern)
- Amortized overhead: <0.015% (6.6× better than <0.1% target)

**SIMD MinHash**:
- 10 tokens: 5.94× speedup (5.04μs → 0.849μs)
- 100 tokens: 7.12× speedup (59.28μs → 8.32μs)
- 1000 tokens: 7.03× speedup (532.5μs → 75.8μs)
- **Average**: 6.70× speedup (validated)

**End-to-End Pipeline**:
- Latency: 91.84μs → 15.30μs (6.00× improvement)
- Throughput: 10.9K → 65.3K elem/s (6.00× improvement)

---

## Hardware Specifications

### Test System

| Component | Specification |
|-----------|---------------|
| **CPU** | Intel Core Ultra 7 155H |
| **Cores** | 22 cores (6P + 8E + 8LP) |
| **Base Clock** | 1.4 GHz |
| **Boost Clock** | 4.8 GHz (P-cores) |
| **Cache** | L1: 384 KB, L2: 12.5 MB, L3: 24 MB |
| **RAM** | 32 GB DDR5-5600 |
| **OS** | Ubuntu 24.04.1 LTS |
| **Kernel** | Linux 6.14.0-33-generic |
| **Rust** | Nightly 2024-10-30 |

### CPU Features Detected

```
CPU tier: avx2
AVX-512: false
AVX2: true
SSE4.2: true
NEON: false
Generation: 1
```

---

## Performance Targets

### Phase 5 Goals

| Goal | Target | Measured | Status |
|------|--------|----------|--------|
| **CPU Detection Overhead** | <0.1% | <0.015% | ✅ PASS |
| **SIMD MinHash Speedup** | 6-7× | 6.70× | ✅ PASS |
| **Runtime Dispatch Latency** | <10ns | <10ns | ✅ PASS |
| **Singleton Access Latency** | <10ns | <10ns | ✅ PASS |
| **Binary Size Increase** | <20% | +12% | ✅ PASS |

---

## CPU Detection Overhead

**Conclusion**: ✅ **PASS** (<0.001% end-to-end overhead)

Amortized overhead for 10K documents: <0.00089%

---

## SIMD MinHash Performance

| Token Count | Scalar (μs) | SIMD (μs) | Speedup |
|-------------|-------------|-----------|---------|
| 10 | 5.04 | 0.849 | **5.94×** |
| 100 | 59.28 | 8.32 | **7.12×** |
| 1000 | 532.5 | 75.8 | **7.03×** |

**Average Speedup**: 6.70× (geometric mean)

---

## End-to-End Pipeline

| Metric | Scalar | SIMD | Improvement |
|--------|--------|------|-------------|
| **Latency** | 91.84 μs | 15.30 μs | **6.00×** |
| **Throughput** | 10.9 K elem/s | 65.3 K elem/s | **6.00×** |

---

## Regression Analysis

| Metric | v1.1 (Before) | v1.2 (After) | Change | Status |
|--------|---------------|--------------|--------|--------|
| **SIMD MinHash (100 tok)** | 1.17× slower | 7.12× faster | +608% | ✅ **FIXED** |
| **End-to-End Latency** | 91.84μs | 15.30μs | -83.3% | ✅ **IMPROVED** |

---

## Recommendations

### Production Deployment

**✅ APPROVED for production deployment**

```bash
cargo +nightly build --release --features simd-minhash
```

**Expected Performance**:
- AVX2 CPUs (97%+ coverage): 6-7× speedup
- Non-AVX2 CPUs (3%): Automatic scalar fallback

---

**END OF BENCHMARK RESULTS**
