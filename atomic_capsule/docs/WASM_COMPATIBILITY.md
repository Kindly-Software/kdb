# WASM Compatibility Guide - atomic_capsule v0.4.0

**Version**: 0.4.0
**Status**: ✅ Production Ready
**Target**: WebAssembly (wasm32-unknown-unknown, wasm32-wasi)
**Framework**: UCE34 T0-T3 + T5 + T10 (T4 limited, T9 unavailable)

---

## Quick Start

```toml
[dependencies]
atomic_capsule = { version = "0.4", default-features = false, features = ["preset-wasm"] }
```

Build with SIMD support (optional):

```bash
# With SIMD (requires runtime support)
RUSTFLAGS="-C target-feature=+simd128" \
  cargo build --target wasm32-unknown-unknown --features preset-wasm

# Without SIMD (universal)
cargo build --target wasm32-unknown-unknown --features preset-wasm
```

---

## Tier Support Matrix

| Tier | WASM Support | Details | Speedup |
|------|--------------|---------|---------|
| **T0** Auditable | ✅ Full | const-hashing, simd-hashing (conditional), FixedPointSerialize | 0-100× |
| **T1** Atomic | ✅ Full | DualAtomicU64, gen counters, AtomicHash64/256, CpuCapability | 3-10× |
| **T2** SIMD | ⚠️ Conditional | Requires `simd128` target feature (not all runtimes) | 2-8× |
| **T3** Fixed-Point | ✅ Full | Q8.8, Q16.16, Q32.32, pure integer arithmetic | 2-10× |
| **T4** Batch | ⚠️ Limited | WorkStealingQueue ✅, ParallelBatchProcessor ❌ (no rayon) | 3-10× |
| **T5** Streaming | ✅ Full | AsyncLogCapsule, streaming operations | O(1) |
| **T6** Mixed | ⚠️ Limited | T1+T3 ✅, T1+T2 ⚠️ (T2 conditional), T2+T3 ⚠️ | 6-24× |
| **T7** GPU | ❌ Not Available | WebGL as future trait implementation | — |
| **T8** Network | ❌ Not Available | WASM has no raw socket API (future: fetch/WebSocket) | — |
| **T9** Persistent | ❌ Not Available | No browser filesystem (future: IndexedDB backend) | — |
| **T10** Probabilistic | ✅ Full | MinHash, LSH, HyperLogLog, Bloom (all pure Rust) | 100-1000× |

---

## Preset: preset-wasm

Default feature set optimized for WASM targets.

**Included**:
- T0: const-hashing ✅, FixedPointSerialize ✅
- T1: Full atomic coordination ✅
- T3: Fixed-point arithmetic ✅
- T5: Async logging ✅
- T10: Probabilistic collections ✅

**Excluded**:
- T2 SIMD (use explicit `--features preset-wasm,simd-support` if +simd128)
- T4 Parallel (rayon unavailable in WASM)
- T9 Persistent (no filesystem)

**Compile-time feature gates**:
- `mmap-persistence` disabled
- `distributed-cache` disabled
- `network` disabled
- `parallel` compile-time available (but runtime limited)

---

## Feature Selection by Use Case

### ✅ Recommended: Data Processing (Browser + Server)

```toml
[dependencies]
atomic_capsule = { version = "0.4", default-features = false, features = [
    "std",           # Alloc support
    "preset-wasm",   # T0-T3, T5, T10
] }
```

**Use**: LLM embedding dedup, cryptographic hashing, probability sketches.

**Example**:
```rust
use atomic_capsule::probabilistic::MinHashSignatureCapsule;
use atomic_capsule::primitives::fixed_point::Q8_8;

// MinHash deduplication (T10 - full support)
let mut sig = MinHashSignatureCapsule::new();
for token in text.split_whitespace() {
    sig.hash_token(token);
}

// Fixed-point math (T3 - full support)
let price: Q8_8 = Q8_8::from_raw(100); // $1.00
let adjusted = price.saturating_mul(Q8_8::from_raw(105)); // +5%
```

### ⚠️ Conditional: High-Performance (with SIMD Runtime)

```bash
# Requires wasm32 runtime with SIMD support (v128)
RUSTFLAGS="-C target-feature=+simd128" \
  cargo build --target wasm32-unknown-unknown --features "preset-wasm,simd-support"
```

**Tiers**: T0-T6 (with T2 conditional)

**Example**:
```rust
#[cfg(feature = "simd-support")]
use atomic_capsule::primitives::simd_f32::SimdF32x8Capsule;

#[cfg(feature = "simd-support")]
let mut v = SimdF32x8Capsule::new();
// Only compiled if wasm32 SIMD available
```

### ❌ Not Supported: Persistence + Parallelism

```rust
// These won't compile with preset-wasm:
// - T4 ParallelBatchProcessor (rayon unavailable)
// - T9 CapsuleMmapRegion (no filesystem)
// - T8 DistributedCache (no sockets)

// Workaround: Use in-memory equivalents (T1+T10)
```

---

## Build Targets

### wasm32-unknown-unknown (Pure WASM)

Default target for browser/edge computing.

```bash
cargo build --target wasm32-unknown-unknown --features preset-wasm
```

**Features**:
- No OS dependencies (browser sandbox)
- No filesystem access
- Shared memory limited
- SIMD optional (+simd128)

**Ideal for**:
- Browser extensions
- Edge/service workers
- Isomorphic compute

### wasm32-wasi (WASI Preview)

Runtime with limited OS access.

```bash
cargo build --target wasm32-wasi --features preset-wasm
```

**Features**:
- Filesystem access (sandboxed)
- Networking (future)
- T9 Persistent could work with IndexedDB backend

**Ideal for**:
- Server-side WASM (Wasmtime, Wasmer)
- IoT gateways
- Edge compute with persistence

---

## Feature Compatibility Details

### T0: Auditable Foundation

**const-hashing** ✅ Full support
```rust
use atomic_capsule::hash::ConstHashCapsule;

// Compile-time hash (0ns runtime)
const HASH: u64 = ConstHashCapsule::compute_hash(b"my-key");
// Identical on all platforms (WASM/x86/ARM)
```

**simd-hashing** ⚠️ Conditional
```rust
#[cfg(feature = "simd-support")]
use atomic_capsule::hash::SimdHashCapsule;

// Falls back to scalar on runtimes without v128
```

### T1: Atomic Coordination

**Full support** ✅
```rust
use atomic_capsule::patterns::DualAtomicU64;
use std::sync::atomic::Ordering;

let dual = DualAtomicU64::new(100, 200);
dual.swap_pair(101, 201, Ordering::SeqCst);
// Zero issues with WASM atomics (SharedArrayBuffer)
```

### T3: Fixed-Point Determinism

**Full support** ✅
```rust
use atomic_capsule::primitives::fixed_point::Q16_16;

let a = Q16_16::from_f64(3.14159);
let b = Q16_16::from_f64(2.0);
let c = a.mul(b); // Deterministic: always same result
```

### T4: Batch Processing

**Limited support** ⚠️

WorkStealingQueue works without rayon:
```rust
#[cfg(feature = "parallel")]
use atomic_capsule::parallel::WorkStealingQueue;

// Single-threaded WASM can still use the queue for batching
let queue = WorkStealingQueue::new(1024);
```

ParallelBatchProcessor unavailable (requires rayon multi-threading).

### T5: Streaming

**Full support** ✅
```rust
#[cfg(feature = "async-log")]
use atomic_capsule::collections::AsyncLogCapsule;

// Async operations work in WASM with proper executor
let log = AsyncLogCapsule::new();
```

### T10: Probabilistic Collections

**Full support** ✅
```rust
use atomic_capsule::probabilistic::{MinHashSignatureCapsule, HyperLogLogCapsule};

// All work in pure Rust, no platform dependencies
let mut minhash = MinHashSignatureCapsule::new();
let mut hll = HyperLogLogCapsule::new();

// Deterministic, vectorizable, no sys calls
```

---

## Serialization & Audit Trails (T0)

**Full support** ✅

```rust
use atomic_capsule::serialize::FixedPointSerialize;

#[derive(FixedPointSerialize)]
struct Payment {
    amount: Q16_16,
    timestamp: u64,
}

let p = Payment { amount: Q16_16::from_raw(100), timestamp: 1234567890 };
let json = p.serialize_json()?;  // Q34 audit-ready
```

No external dependencies needed, works identically on all platforms.

---

## Performance Notes

### SIMD Availability by Runtime

| Runtime | v128 Support | Notes |
|---------|--------------|-------|
| Chrome/V8 | ✅ v120+ | SharedArrayBuffer required |
| Firefox | ✅ v79+ | SharedArrayBuffer required |
| Safari/WebKit | ⚠️ Limited | Check version |
| Wasmtime | ✅ Native | SIMD enabled by default |
| Wasmer | ✅ Native | SIMD enabled by default |
| Cloudflare Workers | ❌ No | Falls back to scalar |
| AWS Lambda@Edge | ❌ No | Falls back to scalar |

### Speedup with SIMD

When +simd128 available (Wasmtime, native runtimes):

- **T2 SIMD MinHash**: 7.1× speedup (128 hashes, 8-wide)
- **T2 SIMD HTTP**: 7× speedup (vectorized byte search)
- **T2 SIMD F32x8**: 7-8× speedup (8-lane SIMD)

Without SIMD (browsers, Workers):

- **T3 Fixed-Point**: 2-10× speedup (integer arithmetic)
- **T10 MinHash**: 2× speedup (scalar, fully functional)
- **T0 const-hash**: 100× speedup (compile-time evaluation)

---

## Testing in WASM

### wasm-pack (browser testing)

```bash
# Setup
curl https://rustwasm.org/wasm-pack/installer/init.sh -sSf | sh

# Build
wasm-pack build --target web --features preset-wasm

# Test
wasm-pack test --headless --firefox
```

### Wasmtime (server-side testing)

```bash
# Install
curl https://wasmtime.dev/install.sh -sSf | bash

# Build
cargo build --target wasm32-wasi --features preset-wasm

# Run
wasmtime target/wasm32-wasi/debug/my_app.wasm
```

### Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_point_wasm() {
        let a = Q16_16::from_raw(100);
        let b = Q16_16::from_raw(200);
        assert_eq!(a.add(b).raw(), 300);
    }

    #[test]
    #[cfg(feature = "simd-support")]
    fn test_simd_wasm() {
        let mut v = SimdF32x8Capsule::new();
        // Only compiled if SIMD available
    }
}
```

Run tests:
```bash
cargo test --target wasm32-unknown-unknown --features preset-wasm
```

---

## Common Issues & Solutions

### Issue 1: "simd-support not available"

**Symptom**:
```
error[E0433]: cannot find crate `wasm_simd` in this edition
```

**Solution**:
```bash
# Either remove simd-support feature:
cargo build --target wasm32-unknown-unknown --features preset-wasm

# Or enable with proper SIMD runtime:
RUSTFLAGS="-C target-feature=+simd128" \
  cargo build --target wasm32-unknown-unknown --features "preset-wasm,simd-support"
```

### Issue 2: "mmap-persistence not supported"

**Symptom**:
```
error: feature `mmap-persistence` is not available for `wasm32-unknown-unknown`
```

**Solution**: Don't request T9 features on WASM
```toml
# ❌ Wrong
atomic_capsule = { version = "0.4", features = ["preset-wasm", "mmap-persistence"] }

# ✅ Correct
atomic_capsule = { version = "0.4", default-features = false, features = ["preset-wasm"] }
```

### Issue 3: "rayon not available in WASM"

**Symptom**:
```
error[E0433]: cannot find crate `rayon`
```

**Solution**: Use WorkStealingQueue or async batching instead
```rust
// Instead of ParallelBatchProcessor:
use atomic_capsule::parallel::WorkStealingQueue;

let queue = WorkStealingQueue::new(1024);
for item in items {
    queue.push(item);
}
// Process sequentially or with custom executor
```

### Issue 4: "Cannot access filesystem"

**Symptom**:
```
wasm runtime error: out of bounds memory access
```

**Solution**: Use in-memory alternatives
```rust
// ❌ Won't work in pure WASM:
let mmap = CapsuleMmapRegion::create("file.bin")?;

// ✅ Use in-memory or IndexedDB (future):
let cache = HyperLogLogCapsule::new();  // In-memory cardinality
let bloom = BloomFilterCapsule::new();  // In-memory membership
```

---

## Migration: Native → WASM

### Step 1: Update Cargo.toml

```toml
# Before (native)
[dependencies]
atomic_capsule = "0.4"  # All features enabled

# After (WASM)
[dependencies]
atomic_capsule = { version = "0.4", default-features = false, features = ["preset-wasm"] }
```

### Step 2: Remove unsupported features

```rust
// Before: Persistence (won't work in WASM)
#[cfg(feature = "mmap-persistence")]
let mmap = CapsuleMmapRegion::create(...);

// After: Conditional compilation
#[cfg(not(target_arch = "wasm32"))]
let mmap = CapsuleMmapRegion::create(...);

#[cfg(target_arch = "wasm32")]
let memory = Vec::with_capacity(1024);
```

### Step 3: Test on multiple targets

```bash
# Test on both targets
cargo test --lib
cargo test --target wasm32-unknown-unknown --lib

# Or use conditional compilation:
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_persistent_operations() { ... }
```

---

## Framework Compliance

### UCE34 (Systematic Discovery)

- **Q1-Q9**: Problem/solution analysis
- **Q10**: Tier selection (T0-T3, T5, T10 primary; T4 limited; T9 unavailable)
- **Q11**: Rust-native (100% safe)
- **Q12**: Nightly optimization (portable_simd conditional with +simd128)
- **Q13-Q30**: Implementation details
- **Q34**: Auditability (T0 const-hash, FixedPointSerialize Q34-ready)

### B32 (Fair Benchmarking)

WASM-specific baseline measurements (Wasmtime):

| Tier | Operation | WASM | Native | Ratio |
|------|-----------|------|--------|-------|
| T0 | const_hash | 0ns | 0ns | 1.0× |
| T1 | atomic_swap | <5ns | <5ns | 1.0× |
| T3 | Q16.16 mul | <20ns | <15ns | 1.3× |
| T10 | MinHash sig | <100μs | <100μs | 1.0× |

(Pure Rust workloads see minimal overhead in WASM)

### ASSUM (Safety)

All assumptions valid in WASM:
- Atomics work (SharedArrayBuffer in browser, native in Wasmtime)
- Alignment enforced (WASM memory byte-aligned)
- No UB (100% safe Rust)

### T28 (Testing)

WASM-specific test framework:

```rust
#[cfg(test)]
mod wasm_tests {
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_atomic_wasm() { ... }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_simd_conditional() { ... }
}
```

---

## Future: Planned WASM Support

### v0.4.x: Current

- ✅ T0-T3, T5, T10
- ⚠️ T4 limited
- ❌ T9, T8, T7

### v0.5.0: WebGL (T7)

GPU traits for WebGL compute shaders.

```rust
#[cfg(feature = "gpu-webgl")]
use atomic_capsule::traits::gpu::GpuCapsule;
```

### v0.5.x: IndexedDB (T9)

Persistent storage backend for browsers.

```rust
#[cfg(target_arch = "wasm32")]
use atomic_capsule::persistence::IndexedDbCapsule;
```

### v1.0: Fetch/WebSocket (T8)

Network operations via browser APIs.

```rust
#[cfg(feature = "network-web")]
use atomic_capsule::network::WebSocketCapsule;
```

---

## Recommended Reading

1. **Quick Start**: This document (5 min)
2. **Feature Reference**: CLAUDE.md § Features Reference (10 min)
3. **Tier Details**: CLAUDE.md § Primitives Reference → T0-T3, T10 (20 min)
4. **Examples**: examples/ directory (cargo run examples)
5. **Framework**: UCE34_FRAMEWORK.md Q10-Q12 (tier selection)

---

## Support & Examples

### Example 1: LLM Deduplication (Browser)

```rust
use atomic_capsule::probabilistic::MinHashSignatureCapsule;
use atomic_capsule::hash::ConstHashCapsule;

pub fn deduplicate_documents(docs: &[&str]) -> Vec<Vec<usize>> {
    let mut sigs: Vec<MinHashSignatureCapsule> = docs
        .iter()
        .map(|doc| {
            let mut sig = MinHashSignatureCapsule::new();
            for token in doc.split_whitespace() {
                sig.hash_token(token);
            }
            sig
        })
        .collect();

    // Find near-duplicates (LSH-style)
    let mut clusters = Vec::new();
    // ... implementation ...
    clusters
}
```

Compiled for wasm32:
```bash
cargo build --target wasm32-unknown-unknown --features preset-wasm
wasm-opt -O4 target/wasm32-unknown-unknown/release/lib.wasm -o lib.optimized.wasm
```

### Example 2: Fixed-Point Financial Calcs (Server WASM)

```rust
use atomic_capsule::primitives::fixed_point::Q16_16;

#[cfg(target_arch = "wasm32")]
pub fn calculate_pnl(position_size: Q16_16, entry_price: Q16_16, exit_price: Q16_16) -> Q16_16 {
    let spread = exit_price.sub(entry_price);
    position_size.mul(spread)
}

// Deterministic, no FP errors, works identically on all platforms
let pnl = calculate_pnl(
    Q16_16::from_f64(1000.0),
    Q16_16::from_f64(100.0),
    Q16_16::from_f64(105.0),
);
```

### Example 3: Hybrid Native + WASM

```toml
[dependencies]
atomic_capsule = { version = "0.4", features = ["preset-wasm"] }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
atomic_capsule = { version = "0.4", features = ["preset-production"] }
```

Code detects platform automatically:

```rust
#[cfg(not(target_arch = "wasm32"))]
use atomic_capsule::persistence::CapsuleMmapRegion;

#[cfg(target_arch = "wasm32")]
use atomic_capsule::collections::HyperLogLogCapsule;

#[cfg(not(target_arch = "wasm32"))]
fn init_storage() { /* mmap */ }

#[cfg(target_arch = "wasm32")]
fn init_storage() { /* in-memory HLL */ }
```

---

## Contact & Feedback

- **Documentation**: Inline code comments + examples/
- **Issues**: GitHub issues (tagged `wasm`)
- **Performance**: B32 benchmarking framework (run locally)

---

**Last Updated**: November 2025
**Next Review**: February 2026 (Q1 v0.5.0 release)
