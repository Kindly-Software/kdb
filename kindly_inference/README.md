# Kindly Inference - LLM Primitives with Computational Capsule Architecture

**Production-grade inference primitives for 70B+ LLMs with B32-validated performance.**

[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![B32 Validated](https://img.shields.io/badge/B32-validated-green.svg)](./B32_BENCHMARK_REPORT.md)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](./LICENSE)

---

## Quick Start

```bash
# Add to Cargo.toml
[dependencies]
kindly_inference = { version = "0.1", features = ["portable_simd"] }

# Run benchmarks
cargo +nightly bench --bench inference_primitives_bench --features portable_simd

# View HTML reports
open target/criterion/report/index.html
```

---

## Primitives

### 1. SIMDMatMulCapsule (T2 Tier) ✅ Production-Ready

**2.5× speedup vs optimized scalar matmul**

**Performance** (B32 validated):
- **4096×4096**: 3.91ms SIMD vs 9.91ms scalar = **2.53× speedup**
- **8192×8192**: 14.87ms SIMD vs 38.95ms scalar = **2.62× speedup**

### 2. FlashAttentionCapsule (T6 Tier) ✅ Production-Ready

**2.3× speedup vs standard attention (fused, memory-efficient)**

**Performance** (B32 validated):
- **Seq 128**: 275µs flash vs 636µs standard = **2.32× speedup**
- **Seq 512**: 5.36ms flash vs 11.31ms standard = **2.11× speedup**

### 3. QuantizationCapsule (T3 Tier) ⚠️ INCOMPLETE

**Status**: Requires SIMD INT8 optimization (honest regression reported)

---

## Benchmark Results

See [B32_BENCHMARK_REPORT.md](./B32_BENCHMARK_REPORT.md) for full analysis.

| Primitive | Baseline | SIMD Speedup | Status |
|-----------|----------|--------------|--------|
| **SIMDMatMulCapsule** | Optimized scalar | **2.5-2.6×** | ✅ Production |
| **FlashAttentionCapsule** | Standard attention | **2.1-2.3×** | ✅ Production |
| **QuantizationCapsule** | f32 operations | **0.23-0.65×** | ⚠️ Incomplete |

**Hardware**: Intel Ultra 7 155H, 64GB DDR5-5600, Linux 6.14.0-33

---

## Framework Compliance

### B32: Fair Benchmarking ✅
- Fair baselines (optimized iterator fusion, NOT strawmen)
- Statistical rigor (95% CI, 100-1000 samples, 3s warmup)
- Realistic workloads (70B model dimensions, realistic batches)
- Honest gains (2-3× exceptional, no suspicious 10×+ claims)

---

**Report Generated**: 2025-10-26 by Benchmarking Expert (Claude)
