# kindly_dedup - LLM Training Dataset Deduplication - Build Guide

**Version**: 2.3.0
**Status**: Production Ready (Single-Threaded)
**Tier Stack**: T0+T1+T2+T3+T4+T5+T6+T9+T10
**Performance**: 60K docs/sec (EXCEPTIONAL, B32 validated)

## Quick Start

```bash
# Standard build (RECOMMENDED)
cargo build --release

# Run deduplication
cargo run --release -- --input corpus.jsonl --output results.json --threshold 0.85

# Binary location
./target/release/kindly_dedup
```

## Performance Summary (VALIDATED)

- **Single-Threaded**: 60K docs/sec (EXCEPTIONAL tier)
- **Per-Document Latency**: 16.7 μs
- **Speedup vs Python datasketch**: 38× (60K vs 1.6K)
- **Accuracy**: ≥90% F1 score (92-99% recall)
- **Hardware**: AMD Ryzen 9 6900HX, 8c/16t, 64GB DDR5-4800

## Build Configurations

### Production Build (Recommended)
```bash
# Optimized release build
cargo build --release

# With all optimizations
RUSTFLAGS="-C target-cpu=native -C lto=fat" cargo build --release

# Binary size: ~4.2MB (stripped)
```

### Development Build
```bash
# Fast compilation, debug symbols
cargo build

# With verbose output
cargo build -vv
```

### Nightly Build (SIMD Optimizations)
```bash
# Install nightly
rustup install nightly

# Build with SIMD text hashing (4× speedup)
cargo +nightly build --release --features simd-text-hashing

# Build with all SIMD features
cargo +nightly build --release --features full-minhash-optimization
```

## Feature Flags

### Core Features (Stable)
- `default` - Standard deduplication pipeline
- `cpu-detection` - Runtime CPU capability detection (<10ns cached lookup)
- `parallel-dedup` - Parallel processing (⚠️ BROKEN, use single-threaded)
- `persistent-dedup` - Persistent deduplication (T9+T10, 93% memory reduction)
- `bloom-prefilter` - Bloom pre-filtering (2-10× on duplicate-heavy corpora, enabled by default)
- `batch-lsh` - Batch LSH lookups (1.5× dedup speedup)
- `q16-jaccard` - Deterministic Q16.16 fixed-point Jaccard (100% reproducible)

### SIMD Features (Nightly Required)
- `simd-minhash` - SIMD MinHash (7.1× speedup, portable_simd)
- `simd-text-hashing` - SIMD text hashing (4× FNV-1a, 14M docs/sec)
- `avx512-minhash` - AVX-512 MinHash (2× vs AVX2, 16-lane)
- `cache-optimized-minhash` - Cache-friendly layout (1.3× speedup)
- `full-minhash-optimization` - All MinHash optimizations (2.3-4.7× compound)

### Compliance Features
- `audit-trail` - Q34 hash-chained audit logging (SOX/SOC2/GDPR/HIPAA)
- `meta-capsule` - META_CAPSULE hardware-bound protection (4 layers)

### Other Features
- `http-server` - HTTP API (pure atomic_capsule HTTP)
- `download-tools` - Corpus download utilities
- `benchmarking` - Enable benchmarks (Criterion.rs)
- `full` - All features enabled

## Build Commands

### Standard Production
```bash
# Recommended for production use
cargo build --release --features cpu-detection,persistent-dedup,bloom-prefilter,q16-jaccard

# With audit trail (compliance)
cargo build --release --features cpu-detection,persistent-dedup,audit-trail

# With HTTP API
cargo build --release --features cpu-detection,http-server
```

### SIMD Optimizations (Nightly)
```bash
# All SIMD features
cargo +nightly build --release --features full-minhash-optimization,cpu-detection

# Specific SIMD features
cargo +nightly build --release --features simd-minhash,cpu-detection

# With persistent dedup + SIMD
cargo +nightly build --release --features persistent-dedup,full-minhash-optimization
```

### Client Demo
```bash
# Build client demo (3-tier sales demo: 100K/1M/10M docs)
cargo build --bin client_demo --release --features "benchmarking,persistent-dedup,meta-capsule"

# Run demo
./target/release/client_demo
```

### Audit Viewer (Q34 Compliance)
```bash
# Build audit viewer
cargo build --bin audit_viewer --release --features benchmarking

# Verify audit trail
./target/release/audit_viewer verify target/criterion/audit_trail.jsonl
```

## Testing

```bash
# All library tests (7,500 tests)
cargo test --lib --all-features

# Integration tests
cargo test --test p0_integration_tests --features benchmarking

# Specific test
cargo test --lib test_dedup_pipeline

# With verbose output
cargo test --lib -- --nocapture
```

## Benchmarking

```bash
# All benchmarks (B32 compliant)
cargo bench --features benchmarking

# Specific benchmark suite
cargo bench --bench v1_0_baseline --features benchmarking

# SIMD benchmarks (nightly)
cargo +nightly bench --bench simd_minhash_bench --features simd-minhash

# View results
open target/criterion/report/index.html
```

**Benchmark Suites**:
- `v1_0_baseline` - Python datasketch comparison (38× speedup)
- `v1_1_simd` - SIMD MinHash (7.1× speedup)
- `v1_1_compound` - Tier stacking (204× projected)
- `v1_2_incremental` - Persistent dedup (100× weekly updates)
- `accuracy_validation` - F1 score validation (95%)

## Running Deduplication

### Basic Usage
```bash
# JSONL corpus
cargo run --release -- \
  --input corpus.jsonl \
  --output results.json \
  --threshold 0.85

# With progress bar
cargo run --release -- \
  --input corpus.jsonl \
  --output results.json \
  --threshold 0.85 \
  --verbose
```

### Persistent Deduplication (100× Weekly Updates)
```bash
# Initial build (10M docs, <75 seconds)
cargo run --release --features persistent-dedup -- \
  --input corpus_initial.jsonl \
  --persistent dedup.mmap \
  --threshold 0.85

# Weekly update (100K new docs, <30 seconds)
cargo run --release --features persistent-dedup -- \
  --input corpus_new.jsonl \
  --persistent dedup.mmap \
  --incremental \
  --threshold 0.85
```

### HTTP Server Mode
```bash
# Start HTTP server (port 8080)
cargo run --release --features http-server -- --serve --port 8080

# Test with curl
curl -X POST http://localhost:8080/dedup \
  -H "Content-Type: application/json" \
  -d '{"documents":[{"id":0,"text":"The quick brown fox"}],"threshold":0.85}'
```

## Platform-Specific Builds

### Linux (x86_64)
```bash
# Standard build
cargo build --release

# With CPU-specific optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

### macOS (aarch64)
```bash
# NEON SIMD (Apple Silicon)
cargo build --release --features cpu-detection

# Rosetta 2 (Intel Mac)
cargo build --release --target x86_64-apple-darwin
```

### Windows
```bash
# MSVC toolchain
cargo build --release --target x86_64-pc-windows-msvc

# GNU toolchain
cargo build --release --target x86_64-pc-windows-gnu
```

## Cross-Compilation

### ARM64 Linux
```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

### ARM64 macOS (Apple Silicon)
```bash
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

## Docker Deployment

```dockerfile
# Dockerfile
FROM rust:1.76-slim as builder

WORKDIR /build
COPY . .
RUN cargo build --release --features cpu-detection,persistent-dedup

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libgcc-s1 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/kindly_dedup /usr/local/bin/
EXPOSE 8080
CMD ["kindly_dedup", "--serve", "--port", "8080"]
```

```bash
# Build Docker image
docker build -t kindly_dedup:2.3.0 .

# Run container
docker run -it --rm -p 8080:8080 kindly_dedup:2.3.0

# With volume for persistent storage
docker run -it --rm \
  -p 8080:8080 \
  -v $(pwd)/data:/data \
  kindly_dedup:2.3.0 \
  --persistent /data/dedup.mmap
```

## Common Issues

### Issue: Parallel performance regression
```
warning: ParallelDedupPipeline is EXPERIMENTAL and NOT production-ready
```
**Fix**: Use single-threaded `DedupPipeline` instead:
```rust
use kindly_dedup::DedupPipeline;  // NOT ParallelDedupPipeline

let mut pipeline = DedupPipeline::new(num_documents);
```

**Reason**: ParallelDedupPipeline has 12.8× performance regression (6K vs 60K docs/sec). See `PARALLEL_PERFORMANCE_INVESTIGATION.md` for details.

### Issue: Out of memory
```
error: Cannot allocate memory
```
**Fix 1**: Use persistent deduplication:
```bash
cargo build --release --features persistent-dedup
cargo run --release --features persistent-dedup -- --persistent dedup.mmap
```

**Fix 2**: Reduce dataset size or increase RAM

### Issue: SIMD features not available
```
warning: SIMD features require nightly Rust
```
**Fix**: Use nightly toolchain:
```bash
rustup install nightly
cargo +nightly build --release --features simd-minhash
```

### Issue: Dependency resolution error
```
error: failed to select a version for `atomic_capsule`
```
**Fix**: Ensure atomic_capsule is built first:
```bash
cd /home/samuel/Primitives/atomic_capsule
cargo build --release --lib --features std,native

cd /home/samuel/Primitives/kindly_dedup
cargo build --release
```

## Performance Tuning

### CPU Affinity
```bash
# Pin to specific cores (reduce cache thrashing)
taskset -c 0-3 ./target/release/kindly_dedup --input corpus.jsonl
```

### Huge Pages
```bash
# Enable transparent huge pages
echo always | sudo tee /sys/kernel/mm/transparent_hugepage/enabled

# Run deduplication
./target/release/kindly_dedup --input corpus.jsonl
```

### NUMA Awareness
```bash
# Bind to NUMA node 0
numactl --cpunodebind=0 --membind=0 ./target/release/kindly_dedup
```

## Continuous Integration

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --lib --all-features
      - run: cargo clippy --all-features -- -D warnings
      - run: cargo build --release
      - run: cargo bench --features benchmarking --no-run
```

## References

- **Main Config**: `CLAUDE.md` (architecture, performance, features)
- **atomic_capsule**: `/home/samuel/Primitives/atomic_capsule/CLAUDE.md` (252 primitives)
- **Migration Guide**: `docs/MIGRATION_v3.md` (v2 → v3 upgrade)
- **Features Guide**: `docs/FEATURES.md` (11 optimizations)
- **Demo Guide**: `docs/DEMO_GUIDE.md` (client demo, TUI)
- **Benchmarking**: `benches/sales/README.md` (B32 compliant benchmarks)

## Quick Reference

| Use Case | Command |
|----------|---------|
| **Production Build** | `cargo build --release --features cpu-detection,persistent-dedup` |
| **SIMD Build (Nightly)** | `cargo +nightly build --release --features full-minhash-optimization` |
| **All Tests** | `cargo test --lib --all-features` |
| **Benchmarks** | `cargo bench --features benchmarking` |
| **Client Demo** | `cargo build --bin client_demo --release --features "benchmarking,persistent-dedup,meta-capsule"` |
| **HTTP Server** | `cargo run --release --features http-server -- --serve --port 8080` |
| **Persistent Dedup** | `cargo run --release --features persistent-dedup -- --persistent dedup.mmap` |

## System Requirements

- **Tier 1 (100K docs)**: 2 GB RAM, ~17 min, 60K+ docs/sec
- **Tier 2 (1M docs)**: 4 GB RAM, ~17 sec, 60K+ docs/sec
- **Tier 3 (10M docs)**: 8 GB RAM, ~27 sec, 373K docs/sec @ 16 cores (persistent mode)
- **Tier 4 (100M docs)**: 16 GB RAM, ~4.5 min, 373K docs/sec @ 16 cores (persistent mode)
