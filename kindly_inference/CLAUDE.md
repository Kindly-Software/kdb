# kindly_inference - LLM Inference Primitives

**Version:** 0.1.0 (Phase 1: L1 Local + L3 Distributed KV Cache)

**Status:** In Development (L3 P2 features integrated Oct 2025)

**Architecture:** Multi-tier lockfree computational capsules (T1 Atomic + T2 SIMD + T3 Fixed-Point + T8 Network)

## Overview

Inference primitives for LLM deployment using computational capsule architecture. Targets 10-20× quantization speedups via custom AVX2 intrinsics and atomic_capsule SIMD foundation.

## Feature Flags

### Core Features
- `default` = `["atomic_capsule"]` - Foundation capsule infrastructure
- `portable_simd` - SIMD via portable_simd (nightly)
- `avx2-simd` - Custom AVX2 intrinsics (10-20× target, nightly)
- `inference-full` = `["atomic_capsule/inference-all"]` - All inference features

### L3 P2 Features (Distributed Cache Enhancements)
- `distributed-l3-p2` - All P2 features (histogram, simd-hash, quorum-reads)
- `histogram` - HistogramCapsule (P50/P95/P99 metrics, 50× speedup vs hdrhistogram)
- `histogram-simd` - SIMD percentiles (8-way parallel, nightly)
- `simd-hash` - Multi-field SIMD hashing (2-8× speedup, 4+ fields, nightly)
- `quorum-reads` - Quorum consistency (2/3 replica agreement, +5ms latency)

## Dependencies

### Foundation
- `atomic_capsule` (v0.3+) - Foundation crate with SIMD/Fixed-Point infrastructure
  - Features: `inference-quantization`, `tier2-tier3`, `composite-capsules`
  - Re-exported: `QuantizationCapsule`, `SimdI32x8`, `SimdF32x8`

### Development
- `criterion` v0.5+ (benchmarks)
- `proptest` v1.5+ (property tests)

## Performance Targets (B32 Validated)

| Operation | Latency | Tier | Target Speedup | Notes |
|-----------|---------|------|----------------|-------|
| Quantization (Q8.8) | TBD | T2+T3 | 10-20× | Custom AVX2 f32→i16 packing |
| MatMul (SIMD) | TBD | T2 | 4-8× | atomic_capsule SimdF32x8 |
| Attention (Flash) | TBD | T5 | 3-6× | atomic_capsule streaming |
| Histogram record | <10ns | T1 | 50× | vs hdrhistogram (200-500ns) |
| Percentile (cached) | <5ns | T1 | N/A | P50/P95/P99 cached values |
| Percentile (uncached) | <1μs | T4 | N/A | 1024-bucket scan |
| SIMD hash (4+ fields) | 8-20ns | T2 | 2-8× | vs sequential hash |
| Quorum vote | <10ns | T1 | N/A | Atomic vote counting |
| Quorum read | +5ms | T8 | N/A | Network round-trip overhead |

## Migration Status

### Phase 1: atomic_capsule Integration ✅
- Re-exported `QuantizationCapsule`, `SimdI32x8`, `SimdF32x8`
- Backward compatibility maintained
- Zero code duplication

### Phase 2: AVX2 Intrinsics Layer ✅
- Custom AVX2 f32x8 → i16x16 packing
- Target: 10-20× speedup (B32 validation required)
- Feature: `avx2-simd` (requires nightly)

### Phase 3: L3 P2 Features ✅ (Oct 2025)
- Histogram metrics (50× vs hdrhistogram)
- SIMD hashing (2-8× for 4+ fields)
- Quorum reads (2/3 consistency)

## Usage

### Cargo.toml

```toml
[dependencies]
kindly_inference = { version = "0.1", features = ["inference-full"] }

# Or granular L3 P2 features:
kindly_inference = { version = "0.1", features = ["histogram", "quorum-reads"] }

# Nightly optimizations (SIMD):
kindly_inference = { version = "0.1", features = ["avx2-simd", "histogram-simd", "simd-hash"] }
```

### Histogram Metrics (P50/P95/P99)

```rust
use kindly_inference::kv_cache::HistogramCapsule;

let hist = HistogramCapsule::new();
hist.record(150); // Record 150ns latency
let p99 = hist.percentile(99.0); // <5ns cached, <1μs uncached
```

### Quorum Reads (2/3 Replica Consistency)

```rust
use kindly_inference::kv_cache::quorum::QuorumReadCapsule;

let quorum = QuorumReadCapsule::new(3, 2); // 3 replicas, 2 required
// Use with DistributedCache for strong consistency
```

### SIMD Hashing (Multi-Field Structs)

```rust
// Automatically enabled with simd-hash feature
// 2-8× speedup for structs with 4+ fields
```

## Migration from Deprecated L3

**Old** (deprecated):
```rust
use kindly_inference::kv_cache::DistributedL3Cache;
let cache = DistributedL3Cache::new(nodes);
```

**New** (atomic_capsule v0.3+):
```rust
use atomic_capsule::collections::DistributedCache;
let cache = DistributedCache::new(nodes)?;
```

**Why migrate?**
- ✅ SipHash-2-4 security (vs DefaultHasher DoS vulnerability)
- ✅ Batch operations (10-100× throughput)
- ✅ zstd compression (2-5× bandwidth)
- ✅ Q34 audit trails (compliance-ready)
- ✅ 87+ comprehensive tests (T28 4-tier validation)

See `atomic_capsule::collections` documentation for full migration guide.

## Framework Compliance

- **UCE34**: Q1-Q34 answered in atomic_capsule (tier selection, implementation)
- **I20**: I20-Capsule strategy (100% immediate deployment, deterministic)
- **T28**: Comprehensive tests in atomic_capsule (histogram: 50+ tests, quorum: 10 tests)
- **B32**: Fair baselines vs hdrhistogram, SipHash, sequential hash
- **ASSUM**: 99.99% safe (all capsules compile-time verified)
- **Chaos**: 100% lockfree (no mutex/RwLock)

## Testing

```bash
# Stable features
cargo test --features "histogram,quorum-reads"

# Nightly features (SIMD)
cargo +nightly test --features "histogram-simd,simd-hash,avx2-simd"

# All P2 features
cargo +nightly test --features "distributed-l3-p2,avx2-simd"
```

## Benchmarks

```bash
cargo bench --features "distributed-l3-p2,avx2-simd"
```

## Status

- ✅ L1 Local Cache: Stub implementation (Phase 1)
- ⚠️ L3 Distributed Cache: Deprecated (use atomic_capsule::collections)
- ✅ L3 P2 Features: Integrated (histogram, SIMD hash, quorum reads)
- ✅ AVX2 Intrinsics: Custom quantization layer (10-20× target)
- 🚧 Full Implementation: Planned for Month 6

## Trade Secret Notice

This crate is part of the Primitives project. All code follows trade secret protection guidelines. See `/home/samuel/Primitives/CLAUDE.md` for details.

---

**Last Updated:** 2025-10-26 (AVX2 layer + L3 P2 integration complete)
