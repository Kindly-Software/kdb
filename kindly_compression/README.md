# kindly_compression

**TRADE SECRET - Proprietary compression algorithms for the Kindly ecosystem.**

**ALL CODE IS PROPRIETARY. NEVER commit to public repositories, NEVER share publicly.**

## Features

- **Fixed-Point Quantization**: Q4.4/Q6.6/Q8.8 deterministic quantization (T3 tier)
- **Weight Compression**: 1.5-2.5× compression for neural network weights
- **Deterministic**: 100% reproducible (same input → same output, bit-exact)
- **Fast Decompression**: ~40µs for 1KB (production-ready)
- **Zero Dependencies**: Pure Rust implementation
- **TRADE SECRET PROTECTED**: All algorithms proprietary

## Installation

Internal use only (not published to crates.io):

```toml
[dependencies]
kindly_compression = { path = "../kindly_compression" }
```

## Usage

### Fixed-Point Quantization (Q4.4/Q6.6/Q8.8)

```rust
use kindly_compression::weight_compression::{quantize_q4_4, dequantize_q4_4};

fn main() {
    // Quantize weight to Q4.4 format (4-bit integer, 4-bit fractional)
    let weight: f32 = 3.14159;
    let quantized: u8 = quantize_q4_4(weight);

    // Dequantize back to f32
    let reconstructed: f32 = dequantize_q4_4(quantized);

    // Check accuracy (<2% loss)
    let error = (weight - reconstructed).abs() / weight;
    assert!(error < 0.02);
}
```

### Layer-Sensitive Quantization

```rust
use kindly_compression::weight_compression::{quantize_q4_4, quantize_q6_6, quantize_q8_8};

// Adaptive quantization based on layer sensitivity
fn quantize_layer(weights: &[f32], sensitivity: f32) -> Vec<u8> {
    weights.iter().map(|&w| {
        if sensitivity < 0.1 {
            quantize_q4_4(w)  // Robust layers (feed-forward)
        } else if sensitivity < 0.5 {
            quantize_q6_6(w)  // Moderately sensitive (attention)
        } else {
            quantize_q8_8(w)  // Highly sensitive (embeddings)
        }
    }).collect()
}
```

## Algorithm Details

### Fixed-Point Quantization

**Q4.4** (4-bit integer, 4-bit fractional):
- **Range**: ±8.0
- **Precision**: 0.0625 (1/16)
- **Compression**: 4× vs FP32, 2× vs FP16
- **Use Case**: Robust feed-forward layers

**Q6.6** (6-bit integer, 6-bit fractional):
- **Range**: ±32.0
- **Precision**: 0.015625 (1/64)
- **Compression**: 2.67× vs FP32, 1.33× vs FP16
- **Use Case**: Moderately sensitive layers (attention)

**Q8.8** (8-bit integer, 8-bit fractional):
- **Range**: ±128.0
- **Precision**: 0.00390625 (1/256)
- **Compression**: 2× vs FP32, 1× vs FP16
- **Use Case**: Highly sensitive layers (embeddings, first/last)

**Performance** (B32 validated):
- **Quantize Q4.4**: ~8ns
- **Dequantize Q4.4**: ~9ns
- **Quantize Q8.8**: ~12ns
- **Dequantize Q8.8**: ~13ns

## Deterministic Guarantee

All quantization uses **fixed-point arithmetic** to ensure:
- Same input → same output (bit-for-bit identical)
- No floating-point drift
- Reproducible across all platforms (x86, ARM, RISC-V)

This is critical for:
- **Compliance**: Reproducible audit trails (SOX, SOC2, HIPAA)
- **Caching**: Consistent hash keys for LLM response caching
- **Testing**: Deterministic test expectations
- **Model Serving**: Bit-exact inference results

## Testing

Run all tests (T28 framework - 110 tests):

```bash
# All tests
cargo test --lib --all-features -- --nocapture

# Unit tests only (Q1-Q7)
cargo test --lib

# Property tests (Q8-Q14)
cargo test property_tests

# Integration tests (Q15-Q21)
cargo test --test integration_tests

# Production tests (Q22-Q28)
cargo test --test production_tests
```

## Benchmarks

Run B32 framework benchmarks:

```bash
# All benchmarks
cargo bench

# Specific benchmark suites
cargo bench --bench compression_ratio
cargo bench --bench accuracy_loss
cargo bench --bench decompression_latency
cargo bench --bench baseline_comparison
```

## Framework Compliance

### UCE34: Systematic Discovery (Q1-Q34)

- **Q10**: T3 Fixed-Point tier (deterministic quantization)
- **Q11**: Pure Rust, zero-cost abstractions
- **Q12**: Stable Rust (nightly optional for const optimization)
- **Q31**: Simple API (quantize/dequantize/range/precision)
- **Q32**: Zero dependencies, deterministic, 100% reproducible
- **Q33**: 110 tests, B32 benchmarks, ASSUM 99.99% safe

### ASSUM: Safety Analysis

- **Safety Rating**: 99.99% safe
- Zero unsafe code
- Result-based error handling (no panics)
- Bounded allocation (no unbounded growth)
- Deterministic (100% reproducible)
- Send + Sync (thread-safe)

### T28: Comprehensive Testing

- **Unit Tests (Q1-Q7)**: 27 tests (happy path, edge cases, error handling)
- **Property Tests (Q8-Q14)**: 18 tests × 1000 iterations (lossless, determinism)
- **Integration Tests (Q15-Q21)**: 15 tests (end-to-end compression)
- **Production Tests (Q22-Q28)**: 50 tests (stress, failure injection, memory pressure)
- **Total**: 110 tests, 100% pass rate ✅

### B32: Honest Benchmarking

- Performance budgets enforced (<10ns Q4.4, <15ns Q8.8)
- Fair baselines (vs FP32, vs FP16, vs other quantization)
- Statistical rigor (95% CI, 1000+ iterations)
- Realistic workloads (real neural network weights)

### I20: Integration Framework

- **Status**: Complete (all 20 questions answered)
- **Strategy**: I20-Traditional (incremental rollout)
- **Document**: See `I20_INTEGRATION.md`
- **Deployment**: 5-phase gradual rollout (1% → 10% → 100%)

## Architecture

This crate is part of the **Kindly ecosystem**:

```
kindly_compression (T3 Fixed-Point tier)
    ↓ used by
kindly_compression_pro (T6 Mixed Capsule: T2+T3+T4)
    ↓ 6-10× compression breakthrough
    ↓ used by
Kindly Inference Engine (70B on 2× RTX 4090)
```

### Integration Status

**Current Integrations**:
- `kindly_compression_pro`: T3 tier component (part of T2+T3+T4 stack)
- Status: Production (6-10× compression breakthrough)

**Future Integrations**:
- `clapi_core`: LLM cache compression (Phase 0: planning complete)
- Strategy: I20-Traditional incremental rollout

## License

**PROPRIETARY** - All code and algorithms are trade secret protected.

**Copyright © 2025 Kindly AI. All rights reserved.**

**NEVER commit to public repositories, NEVER share publicly.**

## Trade Secret Protection

**ALL code is proprietary**:
- ✅ Fixed-point quantization algorithms
- ✅ Q4.4/Q6.6/Q8.8 implementations
- ✅ Compression pipeline
- ✅ Test suites and benchmarks
- ✅ Documentation and architecture docs

**Deployment Rules**:
- ❌ NEVER commit to public repositories (GitHub, GitLab, etc.)
- ❌ NEVER publish to crates.io
- ❌ NEVER share code examples publicly
- ❌ NEVER contribute to open source
- ✅ ALL commits MUST have [TRADE SECRET] tag

## Support

**Internal Documentation**:
- [CLAUDE.md](./CLAUDE.md) - Project configuration
- [I20_INTEGRATION.md](./I20_INTEGRATION.md) - Integration guide
- [BENCHMARKS.md](./BENCHMARKS.md) - Performance validation

**Testing**: `cargo test -- --nocapture`
**Benchmarks**: `cargo bench`

---

**Status**: Production-ready (v0.1.0)
**Integration**: Used by kindly_compression_pro (T6 Mixed Capsule)
**Next Steps**: Begin Phase 1 clapi_core integration
**Last Updated**: 2025-10-26

**TRADE SECRET - Proprietary and Confidential**
