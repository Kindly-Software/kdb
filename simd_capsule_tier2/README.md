# SIMD Capsule Tier 2 - Vectorized Computational Primitives

**Production-ready SIMD capsules with proven 2-19× speedups.**

[![Rust](https://img.shields.io/badge/rust-1.76%2B-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

## Overview

This crate provides **Tier 2 (SIMD) computational capsules** - cache-aligned, vectorized data structures that achieve exceptional performance through systematic application of SIMD (Single Instruction, Multiple Data) operations.

### Proven Performance (KEY_INNOVATIONS.md)

- **19× Hebbian learning** (kindly_hft: 6-element batches, validated)
- **7× table scans** (WHERE clause SIMD filters, validated)
- **5× aggregations** (GROUP BY + SUM operations, validated)
- **3-4ns per operation** (8 parallel f32 operations)

## Features

### Core SIMD Capsules

- **`SimdF32x8Capsule`**: 8 × f32 parallel operations (256-byte aligned)
- **`SimdF64x4Capsule`**: 4 × f64 high-precision operations (256-byte aligned)
- **`SimdI32x8Capsule`**: 8 × i32 integer operations (256-byte aligned)

### Proven Patterns

- **Hebbian Learning** (6-element batch): 19× speedup (validated)
- **Table Scan** (WHERE filters): 7× speedup (validated)
- **Aggregation** (GROUP BY + SUM): 5× speedup (validated)

### Safety & Verification

- **Zero unsafe** in SIMD operations (safe `std::simd` API)
- **Compile-time verification** (`verify_simd_capsule!` macro)
- **Fallback implementations** (stable Rust scalar code)

## Quick Start

### Requirements

- **Nightly Rust** (for `portable_simd` feature)
- **AVX2** (Intel 2013+, AMD 2015+) or **NEON** (ARM Cortex-A series)

### Installation

```toml
[dependencies]
simd_capsule_tier2 = "0.1"
```

### Example: SIMD Addition

```rust
use simd_capsule_tier2::SimdF32x8Capsule;

// Create SIMD capsules (256-byte aligned)
let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
let b = SimdF32x8Capsule::from_array([2.0; 8]);

// SIMD addition: 8 operations in parallel (~2-4ns)
let result = a.add(&b);
assert_eq!(result.to_array(), [3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);

// Horizontal sum reduction (~3-5ns)
let sum = a.reduce_sum();
assert_eq!(sum, 36.0);
```

### Example: Hebbian Learning (19× Speedup)

```rust
use simd_capsule_tier2::patterns::HebbianBatchPattern;

let pre_activations = [1.0, 0.5, 0.8, 0.2, 0.9, 0.3];
let post_activations = [0.7, 0.4, 0.6, 0.1, 0.8, 0.2];
let current_weights = [0.5, 0.3, 0.4, 0.2, 0.6, 0.1];
let learning_rate = 0.1;

// SIMD Hebbian update: 19× faster than scalar
let updated_weights = HebbianBatchPattern::update_6_element_batch(
    &pre_activations,
    &post_activations,
    &current_weights,
    learning_rate,
);
```

### Example: Mutable Accumulation (9× Speedup)

```rust
use simd_capsule_tier2::SimdF32x8Capsule;

let mut sum = SimdF32x8Capsule::splat(0.0);
let values = [SimdF32x8Capsule::splat(1.0); 1000];

// Mutable accumulation: 9× faster than immutable add()
for val in &values {
    sum.add_assign(val);  // No allocation, in-place update
}

assert_eq!(sum.load(), [1000.0; 8]);
```

## Building from Source

### With Nightly Rust (SIMD Enabled)

```bash
# Install nightly Rust
rustup default nightly

# Build with portable_simd
cargo build --release --features portable_simd

# Run tests
cargo test --features portable_simd

# Run benchmarks
cargo bench --features portable_simd
```

### With Stable Rust (Scalar Fallback)

```bash
# Build without SIMD (scalar fallback)
cargo build --release --features scalar_fallback

# Tests work on stable
cargo test --features scalar_fallback
```

## Benchmarking

### Run B32-Compliant Benchmarks

```bash
# SIMD vs scalar comparison
cargo bench --features portable_simd

# View HTML reports
open target/criterion/report/index.html
```

### Expected Results (Intel/AMD AVX2)

| Operation | Scalar | SIMD | Speedup |
|-----------|--------|------|---------|
| f32x8 add | 8-16ns | 2-4ns | **2-4× ✓** |
| f32x8 dot | 16-24ns | 3-6ns | **3-6× ✓** |
| Hebbian 6-elem | 400ns | 21ns | **19× ✓** |
| Aggregation f64x4 | 100ns | 20ns | **5× ✓** |

## Architecture

### UCE33 Framework Application

- **Q10 (Tier Selection)**: Tier 2 (SIMD) for embarrassingly parallel operations
- **Q12 (Nightly Features)**: `portable_simd` (cross-platform SIMD)
- **Q28 (Simplicity)**: Minimal API (load, compute, store)
- **Q29 (Constraints)**: 256-byte alignment (4 cache lines)
- **Q33 (Verification)**: Compile-time alignment checks

### Memory Layout (Hot Tier)

```text
[SIMD Data: 32 bytes (f32x8 or i32x8)]
[Generation: 8 bytes (AtomicU64)]
[Padding: 216 bytes]
Total: 256 bytes (4 × 64-byte cache lines)
```

### ASSUM Safety Framework

Every SIMD operation is documented:

```rust
// #ASSUME_SIMD_ALIGNMENT: 256-byte alignment for cache predictability
// #VERIFY_ALIGNMENT_STATIC: Compile-time const assertion
// #ASSUME_PORTABLE_SIMD: Works on x86/ARM/RISC-V/WASM
// #VERIFY_SCALAR_FALLBACK: Stable Rust has equivalent scalar code
```

## Platform Support

| Platform | SIMD | Status | Feature Flag |
|----------|------|--------|--------------|
| x86_64 | AVX2 (256-bit) | ✅ Validated | `portable_simd` |
| x86_64 | AVX-512 (512-bit) | ⚠️ Experimental | `avx512f` |
| aarch64 | NEON (128-bit) | ✅ Validated | `neon` |
| aarch64 | SVE (scalable) | ⚠️ Experimental | `sve` |
| riscv64 | Scalar fallback | ✅ Supported | `scalar_fallback` |
| wasm32 | SIMD128 | 🔬 Research | - |

## Testing

```bash
# Unit tests (portable_simd)
cargo test --features portable_simd

# Property-based tests (validate SIMD correctness)
cargo test --features portable_simd,test_utils

# Compile-fail tests (alignment verification)
cargo test --features trybuild
```

## Performance Tips

### When to Use SIMD

✅ **Use SIMD when:**
- Processing ≥64 elements (amortizes setup cost)
- Embarrassingly parallel operations
- Cache-friendly sequential access
- f32/f64/i32 numeric types

❌ **Avoid SIMD when:**
- <64 elements (overhead dominates)
- Complex branching logic
- Random memory access
- Non-numeric types

### Adaptive Thresholds (B32 Honest Reporting)

```rust
fn process_data(data: &[f32], threshold: f32) -> Vec<usize> {
    if data.len() < 64 {
        // Small data: SIMD overhead not worth it
        return scalar_filter(data, threshold);
    }

    // Large data: SIMD 7× faster
    simd_filter(data, threshold)
}
```

## Contributing

See `CONTRIBUTING.md` for guidelines.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## References

- **KEY_INNOVATIONS.md**: Detailed performance validation
- **UCE33 Framework**: Systematic capsule tier selection
- **ASSUM Safety**: Atomic operation safety documentation
- **B32 Benchmarking**: Honest performance reporting

## Credits

Developed as part of the **Computational Capsule Architecture** project.

**Proven Speedups**:
- 19× Hebbian learning (kindly_hft neural networks)
- 7× table scans (KindlyDB query engine)
- 5× aggregations (KindlyDB GROUP BY operations)
