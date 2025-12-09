# kindly_dedup Feature Flags

Complete reference for feature flag configuration.

---

## Core Features

### `std` (default)
- **Purpose**: Standard library support
- **Required for**: Collections, file I/O, threading
- **Status**: Required for all builds
- **Dependencies**: None

### `default`
- **Purpose**: Alias for `std`
- **Enables**: Standard library support
- **Use case**: Default cargo builds

---

## Performance Features

### `simd-minhash`
- **Purpose**: SIMD-accelerated MinHash signature computation (T2 SIMD tier)
- **Performance**: 2-8× speedup for 4+ field structs
- **Requirements**: Nightly Rust (`portable_simd` feature)
- **Status**: Phase 2.2 production-ready
- **Use case**: High-throughput signature generation

**Build**:
```bash
cargo +nightly build --features simd-minhash
```

### `parallel-dedup`
- **Purpose**: Multi-threaded deduplication pipeline (T4 Batch tier)
- **Performance**: 8-12× speedup on 8+ cores
- **Primitives**: `atomic_capsule::parallel` (100% lockfree)
- **Status**: Phase 5.0 production-ready
- **Use case**: Large-scale corpus processing (1M+ docs)

**Build**:
```bash
cargo build --features parallel-dedup
```

### `simd-jaccard`
- **Purpose**: SIMD-accelerated Jaccard similarity computation (T2 SIMD tier)
- **Performance**: 7.1× speedup vs scalar baseline (EXCEPTIONAL)
- **Requirements**: Nightly Rust + `simd-minhash`
- **Status**: Validated (integration pending)
- **Use case**: Exact Jaccard validation, ground truth computation

**Build**:
```bash
cargo +nightly build --features simd-jaccard
```

### `parallel-ground-truth`
- **Purpose**: Parallel ground truth generation (T4 Batch tier)
- **Performance**: 8× speedup vs single-threaded
- **Primitives**: `atomic_capsule::parallel`
- **Status**: Production-ready
- **Use case**: Accuracy validation (100K-1M docs)

**Build**:
```bash
cargo build --features parallel-ground-truth
```

### `compound-ground-truth`
- **Purpose**: Compound SIMD + Parallel ground truth (T6 Mixed: T2+T4)
- **Performance**: 24× speedup vs exhaustive O(n²) (BREAKTHROUGH)
- **Enables**: `simd-jaccard` + `parallel-ground-truth`
- **Requirements**: Nightly Rust
- **Status**: Production-ready (client demo validated)
- **Use case**: ExhaustiveCompound accuracy validation (100K docs)

**Build**:
```bash
cargo +nightly build --features compound-ground-truth
```

---

## Protection Features

### `meta-capsule`
- **Purpose**: META_CAPSULE binary protection (4 layers)
- **Layers**:
  1. Build-time: Customer ID embedding + binary signing
  2. Circuit breaker: 8 detection methods (debugger, VM, memory, injection, timing, fault, hardware, voting)
  3. License: DualAtomicU64 + hardware binding + PUF
  4. Audit trail: AtomicHash256 hash-chained Q34 compliance
- **Overhead**: <0.3% (all layers combined)
- **Requirements**: `CUSTOMER_ID` environment variable at build time
- **Status**: Production-ready (v1.5)
- **Use case**: Client demo binary, production deployments

**Build**:
```bash
CUSTOMER_ID=demo-$(uuidgen) cargo build --release --features meta-capsule
```

---

## Utility Features

### `benchmarking`
- **Purpose**: Benchmark infrastructure (Criterion.rs + audit trails)
- **Includes**: Ground truth generation, accuracy validation, Q34 audit logging
- **Required for**: Sales benchmarks, audit viewer binary
- **Status**: Production-ready (Phase 1-5 complete)
- **Use case**: Performance validation, sales demonstrations

**Build**:
```bash
cargo build --features benchmarking
```

**Benchmarks**:
```bash
# v1.0 baseline
cargo bench --bench v1_0_baseline --features benchmarking

# v1.1 SIMD (requires nightly)
cargo +nightly bench --bench v1_1_simd --features benchmarking,simd-minhash

# v1.1 compound (T6 Mixed)
cargo +nightly bench --bench v1_1_compound --features benchmarking,compound-ground-truth
```

### `download-tools`
- **Purpose**: Corpus download utilities (existing binaries)
- **Dependencies**: `reqwest`, `tokio`, `flate2`, `indicatif`, `chrono`, `futures-util`
- **Binaries**: `download_corpus`, `validate_accuracy`, `measure_latency`, `generate_synthetic_corpus`, `stress_test_10m`
- **Status**: Production-ready
- **Use case**: Pre-training corpus acquisition, validation dataset generation

**Build**:
```bash
cargo build --features download-tools
```

### `http-server`
- **Purpose**: HTTP API (pure atomic_capsule HTTP)
- **Performance**: 10-50× vs traditional web frameworks (T8 Network tier)
- **Primitives**: `atomic_capsule::http` (100% lockfree)
- **Status**: Available (not used in demo)
- **Use case**: Production API deployments

**Build**:
```bash
cargo build --bin dedup_server --features http-server
```

---

## Convenience Features

### `full`
- **Purpose**: All features enabled
- **Enables**: `parallel-dedup`, `http-server`, `download-tools`, `simd-minhash`, `benchmarking`, `binary-protection`
- **Use case**: Full capability testing

**Build**:
```bash
cargo +nightly build --all-features
```

---

## Versioned Features (Future)

### `v1_2-persistent`
- **Purpose**: v1.2 incremental deduplication (T9 Persistent tier)
- **Performance**: 100× weekly updates (BREAKTHROUGH, projected)
- **Primitives**: Mmap atomics, MVCC, persistent MinHash cache
- **Status**: Planned (Phase 6)
- **Use case**: Weekly corpus updates (100M+ docs)

---

## Feature Combinations

### Client Demo (Standard)
```bash
cargo build --release --bin client_demo --features "benchmarking"
```
- **Includes**: Ground truth, accuracy validation, basic protection
- **Excludes**: META_CAPSULE layers, SIMD acceleration
- **Use case**: Evaluation demos, development testing

### Client Demo (Protected)
```bash
CUSTOMER_ID=demo-$(uuidgen) cargo build --release --bin client_demo --features "meta-capsule,benchmarking"
```
- **Includes**: All 4 META_CAPSULE layers, Q34 audit trail
- **Excludes**: SIMD acceleration (for stability)
- **Use case**: Production sales demos, paid evaluations

### Client Demo (Maximum Performance)
```bash
CUSTOMER_ID=demo-$(uuidgen) cargo +nightly build --release --bin client_demo --features "meta-capsule,benchmarking,compound-ground-truth"
```
- **Includes**: META_CAPSULE + T6 Mixed (24× ground truth speedup)
- **Requirements**: Nightly Rust
- **Use case**: Maximum performance demonstrations (100K accuracy validation in <10 min)

### Benchmarking Suite
```bash
# Baseline (stable)
cargo build --release --features benchmarking

# SIMD (nightly)
cargo +nightly build --release --features "benchmarking,simd-minhash,simd-jaccard"

# Compound (nightly, T6 Mixed)
cargo +nightly build --release --features "benchmarking,compound-ground-truth"
```

### Production Deployment
```bash
CUSTOMER_ID=[client-uuid] cargo build --release --features "meta-capsule,parallel-dedup"
```
- **Includes**: META_CAPSULE protection + multi-threaded pipeline
- **Excludes**: Development tools, benchmarking
- **Use case**: Production deployments (1M+ docs/day)

---

## Feature Dependencies

```
meta-capsule
  └─ std (required)

benchmarking
  └─ std (required)

compound-ground-truth
  ├─ simd-jaccard
  │   ├─ atomic_capsule/portable_simd (nightly)
  │   └─ simd-minhash
  │       └─ atomic_capsule/portable_simd (nightly)
  └─ parallel-ground-truth
      └─ std (required)

parallel-dedup
  └─ std (required)

download-tools
  ├─ reqwest
  ├─ tokio
  ├─ flate2
  ├─ indicatif
  ├─ chrono
  └─ futures-util

http-server
  ├─ tokio
  ├─ hyper
  ├─ hyper-util
  └─ http-body-util

full
  ├─ parallel-dedup
  ├─ http-server
  ├─ download-tools
  ├─ simd-minhash
  ├─ benchmarking
  └─ binary-protection (deprecated, use meta-capsule)
```

---

## Deprecated Features

### `binary-protection`
- **Status**: DEPRECATED (use `meta-capsule` instead)
- **Reason**: Layer 2 only (circuit breaker), incomplete protection
- **Migration**: Replace with `meta-capsule` for 4-layer defense

---

## Testing Features

All features tested in isolation and combination:

```bash
# Unit tests (all features)
cargo test --all-features

# Integration tests (stable)
cargo test --features "benchmarking"

# SIMD tests (nightly)
cargo +nightly test --features "simd-minhash,simd-jaccard"

# Compound tests (nightly, T6 Mixed)
cargo +nightly test --features "compound-ground-truth"

# META_CAPSULE tests (protected)
CUSTOMER_ID=test-$(uuidgen) cargo test --features "meta-capsule"
```

---

## Performance Summary

| Feature | Speedup | Tier | Status | Requires Nightly |
|---------|---------|------|--------|------------------|
| `simd-minhash` | 2-8× | T2 SIMD | Production | Yes |
| `parallel-dedup` | 8-12× | T4 Batch | Production | No |
| `simd-jaccard` | 7.1× | T2 SIMD | Validated | Yes |
| `parallel-ground-truth` | 8× | T4 Batch | Production | No |
| `compound-ground-truth` | 24× | T6 Mixed | Production | Yes |
| `meta-capsule` | <0.3% overhead | T0+T1 | Production | No |

---

## B32 Validation Status

| Feature | Baseline | Speedup | Classification | Validated |
|---------|----------|---------|----------------|-----------|
| `simd-minhash` | Scalar MinHash | 2-8× | Exceptional | ✅ Phase 2.2 |
| `parallel-dedup` | Single-threaded | 8-12× | Breakthrough | ✅ Phase 5.0 |
| `simd-jaccard` | Scalar Jaccard | 7.1× | Exceptional | ✅ v1.1 |
| `compound-ground-truth` | Exhaustive O(n²) | 24× | Breakthrough | ✅ Client demo |

---

## Questions?

- **Feature not working?** Check dependencies: `cargo tree -f "{p} {f}"`
- **Performance not matching?** Verify nightly: `rustc --version` (should include "-nightly")
- **Build errors?** Check requirements: `meta-capsule` requires `CUSTOMER_ID` env var
- **Contact**: support@kindly.ai
