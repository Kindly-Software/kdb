# atomic_capsule - Build Guide

**Version**: 0.8.0
**Status**: Production-Ready
**Primitives**: 252 capsules across 12 tiers (T0-T11)

## Quick Start

```bash
# Recommended: Native with all const generics (99.996% allocation speedup)
cargo build --release --features preset-native-nightly

# Stable fallback (no const generics)
cargo build --release --features preset-native

# Library only (for dependencies)
cargo build --lib --features std
```

## Presets (MANDATORY - Use These First)

**Nightly Presets** (RECOMMENDED - 99.996% allocation speedup):
```bash
# Native + all const generics (RECOMMENDED)
--features preset-native-nightly

# Embedded + zero-alloc const generics (CRITICAL for deterministic memory)
--features preset-embedded

# WASM + const generics (99.996% alloc speedup)
--features preset-wasm-nightly

# Future Capsule OS + const generics
--features preset-capsule-os
```

**Legacy Presets** (Stable):
```bash
# Standard x86_64/aarch64 development
--features preset-dev

# Production deployment (all tiers + audit)
--features preset-prod

# HFT/low-latency (all tiers + profiling)
--features preset-hft

# Compliance-ready (all tiers + FIPS + Q34 audit)
--features preset-compliance

# Full nightly (all features + max optimization)
--features preset-full-nightly
```

## Platform Targets

### Native (x86_64/aarch64 Linux/macOS/Windows)
```bash
# Standard build
cargo build --release --features preset-native-nightly

# With tokio async runtime
cargo build --release --features preset-native-nightly,tokio-compat

# Test
cargo test --lib --features preset-native-nightly
```

### WASM (Browser)
```bash
# Build for WASM
cargo build --target wasm32-unknown-unknown --features preset-wasm-nightly

# Note: T1/T3/T5/T10 full support, T2/T4/T6 conditional, T7/T8/T9 unavailable
```

### Embedded (ARM Cortex-M, no_std)
```bash
# Embedded build (zero allocation required)
cargo build --target thumbv7em-none-eabihf --features preset-embedded

# Verify no_std compliance
cargo check --target thumbv7em-none-eabihf --no-default-features --features embedded
```

## Feature Flags

### Core Features
- `std` - Standard library (required for most use cases)
- `alloc` - Heap allocation without full std
- `nightly` - Enable nightly features (portable_simd, const_fn_floating_point, atomic_from_mut)
- `derive` - Automatic verification with #[derive(ComputationalCapsule)]

### Const Generics (Nightly Phase 2 - 99.996% Allocation Speedup)
- `nightly-const-generics` - Core const generics (18 primitives, zero-alloc inline arrays)
- `nightly-const-simd` - T2 SIMD + T3 Fixed-Point const (4 primitives: SimdF32x8Const, QuantizerConst, FixedPointMatrixConst, FIRFilterConst)
- `nightly-const-probabilistic` - T10 probabilistic const (3 primitives: BloomFilterConst, HyperLogLogConst, CountMinSketchConst)
- `nightly-const-streaming` - T5 streaming const (3 primitives: PacketBufferConst, StreamingWindowConst, RateLimiterConst)
- `nightly-const-mixed` - T6 mixed const (3 primitives: VectorizedBatchConst, FixedPointSIMDConst, ProbabilisticCacheConst)

### Tier Features
- `atomic-core` - T1 Atomic (DualAtomicU64, generation counters)
- `simd-native` - T2 SIMD (portable_simd, nightly required)
- `fixed-point` - T3 Fixed-Point (Q8.8, Q16.16, Q32.32, Q48.16)
- `parallel` - T4 Batch (lockfree queues, work-stealing)
- `async-log` - T5 Streaming (AsyncLogCapsule, tokio required)
- `composite` - T6 Mixed (tier stacking: T1+T2+T3)
- `probabilistic` - T10 Probabilistic (MinHash, LSH, HyperLogLog, Bloom)

### Collections
- `queue-bounded` - Bounded SPSC/MPMC queues
- `queue-unbounded` - Unbounded lockfree queues
- `cache` - LockfreeCacheCapsule (SipHash, TTL)
- `histogram` - HistogramCapsule (50× vs hdrhistogram)
- `lockfree-btree` - B+ tree (5-10× speedup)

### Security/Compliance
- `const-hashing` - Compile-time FNV-1a (0ns runtime, 100× speedup)
- `audit-q34` - Q34 compliance audit trail (SOX/SOC2/GDPR/HIPAA)
- `crypto-license` - RSA-4096/Ed25519 license validation
- `tpm-binding` - TPM 2.0 hardware binding

## Build Configurations

### Development Build
```bash
# Fast compilation, debug symbols
cargo build --features preset-dev

# With clippy verification
cargo clippy --features preset-dev -- -D warnings
```

### Release Build
```bash
# Optimized binary
cargo build --release --features preset-native-nightly

# With LTO and codegen-units=1 (maximum optimization)
RUSTFLAGS="-C lto=fat -C codegen-units=1" cargo build --release --features preset-native-nightly
```

### Testing
```bash
# All features (266 tests)
cargo test --lib --all-features

# Stable only
cargo test --features preset-dev

# Nightly const generics (138+ tests)
cargo +nightly test --lib --features nightly-const-generics,nightly-const-simd

# Specific tier
cargo test --features std,simd-native,queue-bounded
```

### Benchmarking
```bash
# All benchmarks
cargo bench --all-features

# Specific benchmark
cargo bench --bench concurrent_map_u64_bench --features specialized-u64,portable_simd

# Const generics benchmarks
cargo +nightly bench --bench work_stealing_queue_const_bench --features nightly-const-generics
```

## Toolchain Requirements

### Stable (1.76+)
```bash
# Install stable toolchain
rustup install stable
rustup default stable

# Most features work on stable
cargo build --release --features preset-dev
```

### Nightly (Required for SIMD and Const Generics)
```bash
# Install nightly toolchain
rustup install nightly
rustup default nightly

# Build with nightly features (RECOMMENDED)
cargo +nightly build --release --features preset-native-nightly

# Verify nightly features
cargo +nightly check --features nightly-const-generics
```

## Common Issues

### Issue: Missing `generic_const_exprs` feature
```
error[E0658]: generic const expressions are unstable
```
**Fix**: Use nightly Rust and enable const generics features:
```bash
rustup install nightly
cargo +nightly build --features nightly-const-generics
```

### Issue: `missing_capsule_verification` error
```
error: missing capsule verification
```
**Fix**: Add `#[derive(ComputationalCapsule)]` to your capsule structs:
```rust
use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[repr(C, align(64))]
pub struct MyCapsule {
    data: AtomicU64,
}
```

### Issue: Feature conflict
```
error: Package `atomic_capsule` does not have feature `foo`
```
**Fix**: Use presets instead of individual flags. See `Cargo.toml` for valid features.

### Issue: WASM build errors (T9 Persistent)
```
error: mmap not supported on WASM
```
**Fix**: Use `preset-wasm-nightly` which excludes unavailable tiers:
```bash
cargo build --target wasm32-unknown-unknown --features preset-wasm-nightly
```

## Cross-Compilation

### ARM64 (aarch64-unknown-linux-gnu)
```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu --features preset-native-nightly
```

### RISC-V (riscv64gc-unknown-linux-gnu)
```bash
rustup target add riscv64gc-unknown-linux-gnu
cargo build --release --target riscv64gc-unknown-linux-gnu --features preset-dev
# Note: No T2 SIMD support on RISC-V
```

## Documentation

```bash
# Generate docs
cargo doc --no-deps --all-features --open

# With private items
cargo doc --no-deps --all-features --document-private-items
```

## CI/CD Integration

```yaml
# .github/workflows/ci.yml example
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo +nightly test --lib --features preset-native-nightly
      - run: cargo +nightly clippy --features preset-native-nightly -- -D warnings
```

## Performance Tips

1. **Use nightly presets** for 99.996% allocation speedup (const generics)
2. **Enable LTO** for release builds: `RUSTFLAGS="-C lto=fat"`
3. **Use CPU-specific flags**: `RUSTFLAGS="-C target-cpu=native"`
4. **Prefer presets** over individual features (optimized combinations)
5. **Profile before optimizing**: `cargo flamegraph --release --bin your_binary`

## References

- **Main Config**: `CLAUDE.md` (252 primitives, 81+ features, 7 presets)
- **UCE34 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/xml/frameworks/uce34.xml`
- **Migration Guide**: `docs/MIGRATION_v0.3_v0.4.md`
- **Platform Matrix**: `docs/PLATFORM_MATRIX.md`
- **WASM Compatibility**: `docs/WASM_COMPATIBILITY.md`

## Quick Reference

| Use Case | Command |
|----------|---------|
| **Native Development** | `cargo build --features preset-native-nightly` |
| **WASM Browser** | `cargo build --target wasm32-unknown-unknown --features preset-wasm-nightly` |
| **Embedded (no_std)** | `cargo build --target thumbv7em-none-eabihf --features preset-embedded` |
| **Production Deploy** | `cargo build --release --features preset-prod` |
| **HFT/Low-Latency** | `cargo build --release --features preset-hft` |
| **Compliance (SOX/SOC2)** | `cargo build --release --features preset-compliance` |
| **Testing** | `cargo test --lib --features preset-native-nightly` |
| **Benchmarking** | `cargo bench --all-features` |
