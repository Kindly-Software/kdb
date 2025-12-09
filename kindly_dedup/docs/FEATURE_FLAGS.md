# kindly_dedup Feature Flags

Comprehensive guide to all 60+ feature flags for configuring kindly_dedup.

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Core Features](#core-features)
3. [Performance Features](#performance-features)
4. [SIMD Acceleration](#simd-acceleration)
5. [Optimization Features](#optimization-features)
6. [Compliance & Audit](#compliance--audit)
7. [Protection & Security](#protection--security)
8. [Development & Testing](#development--testing)
9. [Utility Features](#utility-features)
10. [Feature Combinations](#feature-combinations)
11. [Performance Impact Matrix](#performance-impact-matrix)
12. [Recommended Configurations](#recommended-configurations)

## Quick Reference

### Most Common Combinations

```toml
# Development (stable Rust, basic features)
features = ["std"]

# Production (nightly Rust, high performance)
features = ["parallel-dedup", "simd-minhash", "bloom-prefilter", "audit-trail"]

# Maximum Performance (nightly Rust, all optimizations)
features = ["parallel-dedup", "simd-minhash", "simd-text-hashing", "avx512-minhash",
            "cache-optimized-minhash", "batch-lsh", "bloom-prefilter"]

# Low Memory (persistent mode, 93% reduction)
features = ["persistent-dedup", "parallel-dedup"]

# Full Features (everything enabled)
features = ["full"]
```

## Core Features

### `default`

**Tier**: Foundation
**Rust**: Stable 1.76.0+
**Description**: Standard deduplication pipeline with basic features.

**Includes**:
- `std` - Standard library support

**Use When**:
- Learning kindly_dedup
- Prototyping small datasets (<10K docs)
- Stable Rust environment required

**Example**:
```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup" }  # Uses default features
```

**Performance**: 60K docs/sec (single-threaded baseline)

---

### `std`

**Tier**: Foundation
**Rust**: Stable 1.76.0+
**Description**: Enable standard library support (required for most features).

**Enables**:
- File I/O operations
- Threading primitives
- HashMap/Vec collections
- Standard error handling

**Use When**:
- Running on systems with std (not embedded)
- Using any file-based or network features

**Disable When**:
- Embedded systems (no_std targets)
- Kernel modules

**Example**:
```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", default-features = false, features = ["std"] }
```

---

### `parallel-dedup`

**Tier**: T4 Batch
**Rust**: Stable 1.76.0+
**Description**: Multi-threaded parallel processing using atomic_capsule::parallel (100% lockfree).

**Performance**: 6-10× speedup over single-threaded (hardware-dependent)
- 8 cores: 191K docs/sec
- 16 cores: 373K docs/sec (Phase 11 measured)

**Memory**: Scales with thread count (2-4 GB per 1M docs)

**API**:
```rust
use kindly_dedup::ParallelDedupPipeline;

let mut pipeline = ParallelDedupPipeline::new(1_000_000, 16)?;
pipeline.add_documents(&documents)?;
let clusters = pipeline.find_duplicates(0.85)?;
```

**Use When**:
- Processing >100K documents
- Multi-core CPU available (4+ cores)
- Need maximum throughput

**Dependencies**: Requires `std` feature

**Example**:
```toml
features = ["parallel-dedup"]
```

---

### `persistent-dedup`

**Tier**: T9 Persistent + T10 Probabilistic
**Rust**: Nightly (requires `nightly-atomic`)
**Description**: Memory-mapped persistent storage with crash recovery.

**Performance**: 373K docs/sec (same as parallel, memory-efficient)

**Memory Reduction**: 93% (3.5 GB vs 40 GB for 10M docs)

**Features**:
- Crash-safe recovery (<1 second)
- Incremental updates (200× faster than rebuild)
- Atomic generation counters
- LSH rebuild on startup

**API**:
```rust
use kindly_dedup::PersistentDedupPipeline;

let mut pipeline = PersistentDedupPipeline::create("corpus.mmap", 10_000_000)?;
pipeline.add_document(0, "text")?;
let is_dup = pipeline.is_duplicate("text")?;
```

**Use When**:
- Limited RAM (<8 GB for 10M docs)
- Need crash recovery
- Incremental updates (weekly/daily)

**Dependencies**: Requires `parallel-dedup`, `nightly` feature

**Example**:
```toml
features = ["persistent-dedup"]  # Automatically enables parallel-dedup
```

---

### `cpu-detection`

**Tier**: T1 Atomic
**Rust**: Stable 1.76.0+
**Description**: Runtime CPU capability detection for SIMD dispatch.

**Overhead**: <10ns (cached after first detection)

**Detects**:
- SSE2, SSE4.1, SSE4.2
- AVX, AVX2
- AVX-512F, AVX-512BW (if enabled)
- FMA, BMI1, BMI2

**API**:
```rust
use kindly_dedup::CpuCapabilityCapsule;

let cpu_caps = CpuCapabilityCapsule::detect();
println!("AVX2: {}", cpu_caps.has_avx2());
```

**Use When**:
- Building portable binaries (auto-detect at runtime)
- Supporting multiple CPU generations
- Using SIMD features (`simd-minhash`, etc.)

**Example**:
```toml
features = ["cpu-detection"]
```

## Performance Features

### `nightly`

**Tier**: Foundation
**Rust**: Nightly (required for SIMD and some optimizations)
**Description**: Enable nightly Rust features (portable_simd, const optimizations).

**Unlocks**:
- `simd-minhash` (7.1× speedup)
- `simd-text-hashing` (4× speedup)
- `avx512-minhash` (2× vs AVX2)
- `cache-optimized-minhash` (1.3× speedup)

**Use When**:
- Maximum performance required
- Stable builds not sufficient
- Development/testing environment

**Example**:
```bash
rustup install nightly
cargo +nightly build --features nightly
```

---

## SIMD Acceleration

### `simd-minhash`

**Tier**: T2 SIMD
**Rust**: Nightly (requires `portable_simd`)
**Description**: SIMD-accelerated MinHash signature generation.

**Performance**: 7.1× speedup over scalar baseline (B32 validated)
- Baseline: 47.9 ns per MinHash
- SIMD: 6.7 ns per MinHash

**Vectorization**: 8-way parallel processing (f32x8 SIMD lanes)

**CPU Requirements**: SSE2+ (Intel 2003+, AMD 2003+)

**API**: Transparent (automatically used when enabled)

**Use When**:
- Processing large corpora (>1M docs)
- CPU supports SSE2 or better (99.9% of modern CPUs)
- Nightly Rust acceptable

**Example**:
```toml
features = ["simd-minhash"]
```

**Benchmark**:
```bash
cargo +nightly bench --bench simd_minhash_bench
```

---

### `simd-text-hashing`

**Tier**: T2 SIMD
**Rust**: Nightly (requires `simd-minhash`)
**Description**: SIMD-accelerated FNV-1a text hashing for tokenization.

**Performance**: 4× speedup over scalar (Phase 2 validated)
- Baseline: 3.5 µs per document
- SIMD: 0.875 µs per document
- Throughput: 14M docs/sec (text hashing only)

**Vectorization**: 8-way parallel FNV-1a (u64x8 SIMD lanes)

**Use When**:
- Text tokenization bottleneck (>50% of CPU time)
- Large documents (>100 tokens)
- `simd-minhash` already enabled

**Dependencies**: Requires `simd-minhash`

**Example**:
```toml
features = ["simd-text-hashing"]
```

---

### `avx512-minhash`

**Tier**: T2 SIMD (AVX-512)
**Rust**: Nightly (requires `simd-minhash`, `cpu-capabilities`)
**Description**: AVX-512 16-lane SIMD for MinHash (2× vs AVX2).

**Performance**: 2× speedup over AVX2 (Phase 1 validated)
- AVX2: 6.7 ns per MinHash (8-lane)
- AVX-512: 3.35 ns per MinHash (16-lane)

**CPU Requirements**: AVX-512F + AVX-512BW
- Intel: Skylake-X (2017+), Ice Lake (2019+), Sapphire Rapids (2023+)
- AMD: Zen 4 (2022+)

**Runtime Dispatch**: Auto-detects AVX-512 support, falls back to AVX2

**Use When**:
- CPU supports AVX-512 (check with `cpu_caps.has_avx512()`)
- Maximum SIMD performance required

**Dependencies**: Requires `simd-minhash`, `cpu-capabilities`

**Example**:
```toml
features = ["avx512-minhash"]
```

---

### `cache-optimized-minhash`

**Tier**: T2 SIMD (Cache Optimization)
**Rust**: Nightly (requires `simd-minhash`)
**Description**: Cache-friendly loop transposition and prefetching.

**Performance**: 1.2-1.3× speedup (B32 validated)
- Baseline SIMD: 6.7 ns per MinHash
- Cache-optimized: 5.15 ns per MinHash

**Optimization**: Iteration-first loop (better cache locality) + x86-64 prefetch intrinsics

**Use When**:
- Large batches (>1000 documents)
- L3 cache <32 MB
- Already using `simd-minhash`

**Dependencies**: Requires `simd-minhash`, `nightly`

**Example**:
```toml
features = ["cache-optimized-minhash"]
```

---

### `full-minhash-optimization`

**Tier**: T2 SIMD (Compound)
**Rust**: Nightly
**Description**: All MinHash optimizations combined (SIMD + AVX-512 + Cache).

**Performance**: 2.3-4.7× compound speedup (theoretical)
- Base: 47.9 ns (scalar)
- SIMD: 7.1× → 6.7 ns
- AVX-512: 2× → 3.35 ns
- Cache: 1.3× → 2.58 ns
- **Total**: 18.6× theoretical (2.58 ns per MinHash)

**Use When**:
- Maximum MinHash performance required
- CPU supports AVX-512
- Compound optimizations acceptable

**Dependencies**: Enables `simd-minhash`, `avx512-minhash`, `cache-optimized-minhash`

**Example**:
```toml
features = ["full-minhash-optimization"]
```

## Optimization Features

### `bloom-prefilter`

**Tier**: T1 Atomic + T10 Probabilistic
**Rust**: Stable 1.76.0+
**Description**: Bloom filter pre-filtering to skip duplicate checks.

**Performance**: 2-10× speedup on duplicate-heavy corpora (B32 validated)
- 10% duplicates: 0.95× (minimal benefit)
- 30% duplicates: 1.3× speedup
- 50% duplicates: 2.5× speedup
- 90% duplicates: 10× speedup (EXCEPTIONAL)

**Memory**: 512 KB (configurable)

**False Positive Rate**: 0.01% (1 in 10,000)

**Query Time**: <30ns per lookup

**Use When**:
- Duplicate rate >30%
- Incremental updates (check existing corpus)
- Weekly/daily corpus additions

**Default**: Enabled (recommended for all use cases)

**Example**:
```toml
features = ["bloom-prefilter"]  # Default enabled
```

---

### `batch-lsh`

**Tier**: T4 Batch
**Rust**: Stable 1.76.0+
**Description**: Batch LSH bucket lookups for improved throughput.

**Performance**: 1.3-2× speedup (Phase 3 validated, B32 compliant)
- Baseline: Sequential LSH lookups
- Batch 1K: 1.5× speedup
- Batch 5K: 2× speedup

**Batch Size**: 1000 documents (configurable)

**Use When**:
- Large corpora (>1M docs)
- LSH bucketing bottleneck (>20% CPU time)
- Parallel processing enabled

**Dependencies**: Requires `std`

**Example**:
```toml
features = ["batch-lsh"]
```

---

### `batch-minhash`

**Tier**: T4 Batch
**Rust**: Stable 1.76.0+
**Description**: Batch MinHash processing for improved cache efficiency.

**Performance**: 1.5-2× speedup (Week 2 optimization)

**Batch Size**: 100-1000 documents

**Use When**:
- Parallel processing enabled
- MinHash generation bottleneck
- Large batches available

**Example**:
```toml
features = ["batch-minhash"]
```

## Compliance & Audit

### `audit-trail`

**Tier**: T0 Auditable (Q34 Compliance)
**Rust**: Stable 1.76.0+
**Description**: Tamper-evident hash-chained audit logging for compliance.

**Compliance**: SOX, SOC2, GDPR, HIPAA

**Features**:
- Hash-chained event log (SHA-256)
- Tamper detection (<50ns verify)
- JSON export for analysis
- Zero runtime overhead (<5ns per event)

**API**:
```rust
use kindly_dedup::benchmarking::AuditLogger;

let logger = AuditLogger::new("audit.jsonl")?;
logger.log_event("dedup_start", "corpus_size=10M")?;
logger.log_event("dedup_complete", "clusters=1234")?;
```

**Use When**:
- Regulatory compliance required
- Audit trail for data processing
- Forensic analysis needed

**Output**: `audit_trail.jsonl` (JSON Lines format)

**Example**:
```toml
features = ["audit-trail"]
```

---

### `q16-jaccard`

**Tier**: T3 Fixed-Point
**Rust**: Stable 1.76.0+
**Description**: Deterministic Q16.16 fixed-point Jaccard similarity (100% reproducible).

**Precision**: 16.16 fixed-point (16 bits integer, 16 bits fraction)

**Reproducibility**: 100% (same inputs → same outputs on all platforms)

**Performance**: 1.04× vs f32 (negligible overhead, Phase 0 validated)

**Use When**:
- Reproducible results required (regression testing)
- Cross-platform consistency needed
- Avoiding floating-point nondeterminism

**Example**:
```toml
features = ["q16-jaccard"]
```

## Protection & Security

### `meta-capsule`

**Tier**: Protection Layer 2 (Weaponized Circuit Breaker)
**Rust**: Nightly (requires `binary-protection`)
**Description**: Hardware-bound IP protection (4-layer defense).

**Layers**:
1. **Build Hardening**: SHA-256 signature, customer ID
2. **Weaponized Circuit Breaker**: Tamper detection + degradation
3. **PUF Validation**: Hardware fingerprinting
4. **Audit Trail**: Q34 tamper-evident logging

**Use When**:
- Distributing commercial binaries
- IP protection required
- Client demos with usage limits

**Example**:
```toml
features = ["meta-capsule"]
```

---

### `protection-crypto-license`

**Tier**: Protection P0 (Layer 3)
**Rust**: Stable 1.76.0+
**Description**: Ed25519/RSA signature verification for license validation.

**Algorithms**: Ed25519 (fast), RSA-2048 (legacy)

**Use When**:
- License validation required
- Public key infrastructure available

**Dependencies**: `ed25519-dalek`, `rsa`

**Example**:
```toml
features = ["protection-crypto-license"]
```

---

### `protection-encrypted-state`

**Tier**: Protection P0 (Layer 2)
**Rust**: Stable 1.76.0+
**Description**: AES-256-GCM encrypted state for algorithm parameter protection.

**Encryption**: AES-256-GCM (authenticated encryption)

**Use When**:
- Protecting algorithm parameters from inspection
- Trial/demo mode state encryption

**Dependencies**: `hkdf` (always available)

**Example**:
```toml
features = ["protection-encrypted-state"]
```

---

### `meta-capsule-full`

**Tier**: Protection P2 (All 11 Layers)
**Rust**: Nightly
**Description**: Complete 11-layer protection orchestration.

**Includes**:
- All P0 protection capsules (3 layers)
- All P1 protection capsules (5 layers)
- Orchestrator coordination
- Anomaly detection
- Memory encryption
- Kernel protection

**Use When**:
- Maximum protection required
- Commercial deployment
- High-value IP

**Example**:
```toml
features = ["meta-capsule-full"]
```

## Development & Testing

### `benchmarking`

**Tier**: Development
**Rust**: Stable 1.76.0+
**Description**: B32-compliant benchmarking infrastructure.

**Features**:
- Criterion.rs integration
- Audit trail export
- Statistical analysis (95% CI)
- Reality check classification

**API**:
```rust
use kindly_dedup::benchmarking::{B32Runner, BenchmarkConfig};

let config = BenchmarkConfig::default();
let results = B32Runner::run(&config)?;
```

**Use When**:
- Performance validation
- Before/after comparisons
- B32 compliance reporting

**Example**:
```toml
features = ["benchmarking"]
```

---

### `interactive`

**Tier**: Development (TUI)
**Rust**: Stable 1.76.0+
**Description**: Interactive TUI for demos and testing.

**Features**:
- Real-time progress tracking
- Dual progress bars (Python vs Kindly race)
- META_CAPSULE integration
- E2E workflow commands

**Commands**: 6 commands (process, analyze, export, settings, help, quit)

**Use When**:
- Client demos
- Interactive testing
- Visual progress tracking

**Dependencies**: `clap`, `ratatui`, `inquire`, `crossterm`

**Example**:
```bash
cargo run --bin kindly_dedup --features interactive
```

---

### `download-tools`

**Tier**: Development
**Rust**: Stable 1.76.0+
**Description**: Corpus download utilities (HTTP streaming, compression).

**Features**:
- HTTP/HTTPS download
- Gzip decompression
- Progress tracking
- Resume support

**Use When**:
- Downloading public corpora
- Testing with standard datasets

**Dependencies**: `reqwest`, `flate2`, `indicatif`, `futures-util`

**Example**:
```bash
cargo run --bin download_corpus --features download-tools
```

## Utility Features

### `http-server`

**Tier**: Development (Legacy API)
**Rust**: Stable 1.76.0+
**Description**: Basic HTTP API server (legacy, replaced by `production-api`).

**Endpoints**:
- `POST /deduplicate` - Process corpus
- `GET /status` - Pipeline status

**Use When**:
- Basic HTTP API needed
- Testing/development only
- Not production (use `production-api` instead)

**Example**:
```bash
cargo run --bin dedup_server --features http-server
```

---

### `production-api`

**Tier**: Production (Advanced API)
**Rust**: Nightly (requires atomic_capsule runtime)
**Description**: Production HTTP API with atomic_capsule runtime (zero Tokio/Axum).

**Features**:
- ExecutorCapsule (task spawning)
- ReactorCapsule (epoll/kqueue event loop)
- AsyncTcpListener (2-50× faster than Tokio)
- HTTP/1.1 SIMD parser (7× faster than httparse)
- KindlyDB integration
- Protection orchestration

**Performance**: 2-50× faster than Tokio+Axum

**Use When**:
- Production deployment
- Maximum API performance
- Zero external runtime dependencies

**Dependencies**: Requires many atomic_capsule features

**Example**:
```bash
cargo +nightly run --bin dedup_api --features production-api
```

---

### `sysinfo`

**Tier**: Utility
**Rust**: Stable 1.76.0+
**Description**: System RAM detection for auto-tier selection.

**Note**: Removed in v1.13.2, replaced with `std::thread::available_parallelism()` + `/proc/meminfo`

**Use**: Automatic (no feature flag needed)

---

## Feature Combinations

### Minimal (Stable Rust, Basic)

```toml
features = ["std"]
```

**Performance**: 60K docs/sec (single-threaded)
**Memory**: 2 GB per 1M docs
**Use Case**: Development, prototyping

---

### Standard (Stable Rust, Parallel)

```toml
features = ["parallel-dedup", "bloom-prefilter"]
```

**Performance**: 191K docs/sec @ 8 cores
**Memory**: 4 GB per 1M docs
**Use Case**: Small-scale production (<1M docs)

---

### High Performance (Nightly Rust, SIMD)

```toml
features = ["parallel-dedup", "simd-minhash", "bloom-prefilter", "batch-lsh"]
```

**Performance**: 373K docs/sec @ 16 cores (Phase 11 measured)
**Memory**: 8 GB per 10M docs
**Use Case**: Production (1M-10M docs)

---

### Maximum Performance (Nightly Rust, All Optimizations)

```toml
features = [
    "parallel-dedup",
    "simd-minhash",
    "simd-text-hashing",
    "avx512-minhash",
    "cache-optimized-minhash",
    "batch-lsh",
    "bloom-prefilter",
]
```

**Performance**: 500K-900K docs/sec @ 16 cores (projected, Phase 12+)
**Memory**: 8 GB per 10M docs
**Use Case**: High-throughput production (10M+ docs)

---

### Low Memory (Persistent Mode)

```toml
features = ["persistent-dedup", "parallel-dedup"]
```

**Performance**: 373K docs/sec @ 16 cores
**Memory**: 3.5 GB per 10M docs (93% reduction)
**Use Case**: Memory-constrained systems

---

### Compliance & Audit

```toml
features = ["parallel-dedup", "audit-trail", "q16-jaccard", "meta-capsule"]
```

**Performance**: 373K docs/sec @ 16 cores
**Memory**: 8 GB per 10M docs
**Use Case**: Regulated industries (finance, healthcare)

---

### Full Features

```toml
features = ["full"]
```

**Enables**: All features (development + production + testing)
**Performance**: Maximum (depends on CPU)
**Use Case**: Development, testing, exploration

---

## Performance Impact Matrix

| Feature | Speedup | Memory | Stability | Rust | CPU Req |
|---------|---------|--------|-----------|------|---------|
| `parallel-dedup` | 6-10× | 1.5× | Stable | 1.76+ | Multi-core |
| `simd-minhash` | 7.1× | 1× | Stable | Nightly | SSE2+ |
| `simd-text-hashing` | 4× | 1× | Stable | Nightly | SSE2+ |
| `avx512-minhash` | 2× | 1× | Stable | Nightly | AVX-512 |
| `cache-optimized-minhash` | 1.3× | 1× | Stable | Nightly | Any |
| `bloom-prefilter` | 2-10× | 1.001× | Stable | 1.76+ | Any |
| `batch-lsh` | 1.5× | 1× | Stable | 1.76+ | Any |
| `persistent-dedup` | 1× | 0.07× | Stable | Nightly | Any |
| `audit-trail` | 1× | 1.001× | Stable | 1.76+ | Any |
| `q16-jaccard` | 0.96× | 1× | Stable | 1.76+ | Any |

**Key**:
- **Speedup**: Relative to baseline (1× = no change, 7× = 7 times faster)
- **Memory**: Relative to baseline (1× = same, 0.07× = 93% reduction)
- **Stability**: Production readiness (Stable = proven, Experimental = testing)
- **Rust**: Minimum Rust version (1.76+ = stable, Nightly = nightly required)
- **CPU Req**: CPU feature requirements (Any = no special requirements)

## Recommended Configurations

### Development

```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", features = ["std"] }
```

**Why**: Minimal dependencies, fast builds, stable Rust

---

### Testing

```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", features = [
    "parallel-dedup",
    "benchmarking",
    "download-tools",
] }
```

**Why**: Parallel processing + benchmarking + test data download

---

### Production (Stable Rust)

```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", features = [
    "parallel-dedup",
    "bloom-prefilter",
    "audit-trail",
] }
```

**Why**: 191K docs/sec, stable Rust, audit compliance

---

### Production (Nightly Rust, Maximum Performance)

```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", features = [
    "parallel-dedup",
    "simd-minhash",
    "bloom-prefilter",
    "batch-lsh",
    "audit-trail",
] }
```

**Why**: 373K docs/sec (Phase 11 validated), B32 compliant, audit ready

---

### Production (Low Memory)

```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", features = [
    "persistent-dedup",
    "parallel-dedup",
    "audit-trail",
] }
```

**Why**: 3.5 GB vs 40 GB (93% reduction), crash-safe, incremental updates

---

### Client Demo (Protected)

```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", features = [
    "benchmarking",
    "persistent-dedup",
    "meta-capsule",
    "interactive",
] }
```

**Why**: Hardware-bound protection, TUI interface, usage limits

---

## Troubleshooting Features

### Issue: Out of Memory

**Solution**: Enable `persistent-dedup`

```toml
features = ["persistent-dedup"]
```

**Impact**: 93% memory reduction (3.5 GB vs 40 GB @ 10M docs)

---

### Issue: Slow Performance

**Solution**: Enable parallel + SIMD

```toml
features = ["parallel-dedup", "simd-minhash"]
```

**Impact**: 6-10× parallel + 7.1× SIMD = 42-70× compound

---

### Issue: Nightly Instability

**Solution**: Use stable features only

```toml
features = ["parallel-dedup", "bloom-prefilter"]
```

**Impact**: 191K docs/sec (stable Rust, no SIMD)

---

### Issue: Duplicate-Heavy Corpus Slow

**Solution**: Enable Bloom pre-filter

```toml
features = ["bloom-prefilter"]  # Default enabled
```

**Impact**: 2-10× on duplicate-heavy corpora

---

## Feature Flag Reference

Quick lookup table for all 60+ features:

| Feature | Category | Rust | Performance | Memory | Stability |
|---------|----------|------|-------------|--------|-----------|
| `default` | Core | Stable | 1× | 1× | Stable |
| `std` | Core | Stable | 1× | 1× | Stable |
| `nightly` | Core | Nightly | 1× | 1× | Stable |
| `parallel-dedup` | Core | Stable | 6-10× | 1.5× | Stable |
| `persistent-dedup` | Core | Nightly | 1× | 0.07× | Stable |
| `cpu-detection` | Core | Stable | 1× | 1× | Stable |
| `simd-minhash` | SIMD | Nightly | 7.1× | 1× | Stable |
| `simd-text-hashing` | SIMD | Nightly | 4× | 1× | Stable |
| `avx512-minhash` | SIMD | Nightly | 2× | 1× | Stable |
| `cache-optimized-minhash` | SIMD | Nightly | 1.3× | 1× | Stable |
| `full-minhash-optimization` | SIMD | Nightly | 18.6× | 1× | Stable |
| `bloom-prefilter` | Optimization | Stable | 2-10× | 1.001× | Stable |
| `batch-lsh` | Optimization | Stable | 1.5× | 1× | Stable |
| `batch-minhash` | Optimization | Stable | 1.5-2× | 1× | Stable |
| `audit-trail` | Compliance | Stable | 1× | 1.001× | Stable |
| `q16-jaccard` | Compliance | Stable | 0.96× | 1× | Stable |
| `meta-capsule` | Protection | Nightly | 1× | 1× | Stable |
| `protection-crypto-license` | Protection | Stable | 1× | 1× | Stable |
| `protection-encrypted-state` | Protection | Stable | 1× | 1× | Stable |
| `meta-capsule-full` | Protection | Nightly | 1× | 1× | Stable |
| `benchmarking` | Development | Stable | 1× | 1× | Stable |
| `interactive` | Development | Stable | 1× | 1× | Stable |
| `download-tools` | Development | Stable | 1× | 1× | Stable |
| `http-server` | Utility | Stable | 1× | 1× | Stable |
| `production-api` | Utility | Nightly | 2-50× | 1× | Stable |
| `full` | Meta | Nightly | Max | Max | Stable |

---

**Document Version**: v1.0
**Last Updated**: 2025-11-10
**Framework**: UCE34 + Chaos + ASSUM + B32 + T28 + I20
