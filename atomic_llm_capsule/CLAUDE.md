# CLAUDE.md - Atomic LLM Capsule Project Configuration

**Project-Specific Computational Capsule Patterns for LLM Quantization + Deterministic Training**

Version: 0.2.0 (Vision: Deterministic LLM Training via Fixed-Point GPU Kernels)
Architecture: Tier 2/3/7 Computational Capsules (SIMD + Fixed-Point + GPU)
Foundation: `/home/samuel/Primitives/atomic_capsule/`

---

## 🚀 VISION: Deterministic LLM Training (NEW)

**Status**: Phase 0.1 Validated (kindly_dedup), Phase 0.2-1.0 Planned

### What This Enables

**First production system for 100% deterministic, compliance-ready LLM training**:
- ✓ **Bit-exact reproducibility** across all platforms (NVIDIA/AMD/Apple/Intel)
- ✓ **Q34-compliant model lineage** (hash-chained audit trails)
- ✓ **Regulatory-ready** (EU AI Act, copyright compliance, SOX/SOC2/GDPR/HIPAA)
- ✓ **Zero performance overhead** (Phase 0.1: 1.04× faster than f32)

### Phase 0.1 Validation ✓ (Complete)

**kindly_dedup Q16.16 Jaccard similarity**:
- Q16.16: 58.86 ns
- f32 baseline: 61.12 ns
- **Speedup: 1.04×** (4% faster, MARGINAL, zero overhead)
- **100% deterministic** (bit-exact across all platforms)
- **Q34 compliant** (hash-chained audit trails)

**Key insight**: Fixed-point can match/beat floating-point for real workloads.

### Roadmap

| Phase | Goal | Duration | Status |
|-------|------|----------|--------|
| **0.1** | Q16.16 validation (kindly_dedup) | Complete | ✓ (1.04× speedup) |
| **0.2** | Q16.16 CPU matmul + SIMD | 2-4 weeks | Planned |
| **0.3** | Custom CUDA/ROCm kernels (T7) | 4-8 weeks | Planned |
| **0.4** | Gradient + optimizer | 4-6 weeks | Planned |
| **1.0** | Full 7B LLM training | 12-16 weeks | Planned |

### Documentation

- **Vision**: `docs/DETERMINISTIC_LLM_TRAINING.md` (comprehensive overview, market positioning)
- **Technical Roadmap**: `docs/FIXED_POINT_GPU_ROADMAP.md` (implementation details, code examples)

---

## Mandatory Reading

**CRITICAL**: All agents working on this project must read the following documents before any implementation:

### 1. The Computational Capsule Architecture
- **Source**: `/home/samuel/Docs/The Computational Capsule.md`
- **Priority**: CRITICAL
- **Description**: 6-tier capsule architecture and one-read decision principle
- **Key Concepts**:
  - One-read decisions (eliminate pointer chasing)
  - Co-located metadata (scale/zero with data)
  - Cache alignment patterns (64B/128B/256B)
  - Commit-flip protocol (lockfree atomic updates)

### 2. Atomic Capsule Foundation
- **Source**: `/home/samuel/Primitives/atomic_capsule/`
- **Priority**: CRITICAL
- **Description**: Foundation crate for all capsule implementations
- **Key Components**:
  - Tiered alignment traits (HotTier/WarmTier/ColdTier)
  - Verification macros (`verify_capsule!`, `verify_alignment!`)
  - Retry policies (exponential backoff)
  - Architecture detection (portable cache line sizes)

### 3. Framework Documentation
- **UCE33**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE32_FRAMEWORK.md`
  - Q33: How do atomic capsules fundamentally transform this problem?
- **ASSUM**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/ASSUM_SAFETY.md`
  - Safety assumption validation for unsafe code and atomics
- **B32**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`
  - Performance validation with 95% CI, realistic baselines
- **I20**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/I20_INTEGRATION_FRAMEWORK.md`
  - Systematic integration with existing components
- **T28**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/T28_TESTING_FRAMEWORK.md`
  - Comprehensive testing strategy

---

## Project-Specific Capsule Patterns

### Pattern 1: Micro-Block Co-Located Quantization (MBCQ)

**Problem**: Traditional quantization suffers from pointer chasing (3 cache misses).

**Solution**: Co-locate scale/min with quantized values in single cache line.

**Implementation**:

```rust
#[repr(C)]
struct MicroBlock {
    scale_f16: u16,      // Bytes 0-1: f16 scale factor
    min_f16: u16,        // Bytes 2-3: f16 minimum value
    values_4bit: [u8; 4] // Bytes 4-7: 8 packed 4-bit values
}

#[repr(C, align(64))]
pub struct MicroBlockQuantCapsule {
    blocks: [MicroBlock; 8],  // 64 bytes (8 blocks × 8 bytes)
    generation: AtomicU32,     // 4 bytes
    _padding: [u8; 60]         // 60 bytes (total 128 due to alignment)
}
```

**Key Innovation**: All metadata and data in single cache line read.

**Performance**: 3× faster dequantization (35ns vs 105ns).

**File**: `src/primitives/quant_microblock.rs`

---

### Pattern 2: Tiered Cache-Aligned Capsules

**Problem**: Uniform storage wastes cache on cold weights.

**Solution**: Three capsule types aligned to CPU cache hierarchy.

**Implementation**:

```rust
// Hot tier: L1 cache (64 bytes)
#[repr(C, align(64))]
pub struct HotWeightCapsule {
    weights_q8: [u16; 24],  // 48 bytes (24 Q8.8 weights)
    generation: AtomicU32,   // 4 bytes
    checksum: u16,           // 2 bytes
    _padding: [u8; 10]       // 10 bytes
}

// Warm tier: L2 cache (128 bytes)
#[repr(C, align(128))]
pub struct WarmWeightCapsule {
    weights_q8: [u16; 56],  // 112 bytes (56 Q8.8 weights)
    generation: AtomicU32,   // 4 bytes
    checksum: u16,           // 2 bytes
    _padding: [u8; 10]       // 10 bytes
}

// Cold tier: L3 cache (256 bytes)
#[repr(C, align(256))]
pub struct ColdWeightCapsule {
    weights_q8: [u16; 120], // 240 bytes (120 Q8.8 weights)
    generation: AtomicU32,   // 4 bytes
    checksum: u16,           // 2 bytes
    _padding: [u8; 10]       // 10 bytes
}
```

**Key Innovation**: Type-level tier enforcement, cache-aligned access.

**Performance**: 3.5× faster via cache hierarchy (10ns weighted average).

**File**: `src/primitives/quant_tiered.rs`

---

### Pattern 3: Commit-Flip Lockfree Updates

**Problem**: Locks block readers during quantization parameter updates.

**Solution**: Generation counter commit-flip protocol.

**Implementation**:

```rust
#[repr(C, align(128))]
pub struct AdaptiveQuantCapsule {
    metadata: AtomicU64,  // generation:32 | scale:16 | zero:8 | reserved:8
    weights_4bit: [u8; 64],
    running_min: AtomicU32,
    running_max: AtomicU32,
    access_count: AtomicU32,
    _padding: [u8; 44]
}

impl AdaptiveQuantCapsule {
    pub fn adapt_quantization(&self, weights: &[f32; 128]) {
        // Phase 1: Mark in-progress (odd generation)
        let odd_gen = (self.generation() + 1) | 1;
        let metadata_odd = pack_metadata(odd_gen, scale, zero);
        self.metadata.store(metadata_odd, Ordering::Relaxed);

        // Phase 2: Update payload
        self.quantize_weights(weights, scale);

        // Phase 3: Commit (even generation)
        let even_gen = odd_gen + 1;
        let metadata_even = pack_metadata(even_gen, scale, zero);
        self.metadata.store(metadata_even, Ordering::Release);
    }

    pub fn load_weight(&self, index: usize) -> Option<f32> {
        let metadata = self.metadata.load(Ordering::Relaxed);
        let generation = (metadata & 0xFFFF_FFFF) as u32;

        // Check committed (even generation)
        if generation % 2 != 0 {
            return None;  // In-progress, retry
        }

        // Extract scale/zero and dequantize
        let scale = extract_scale(metadata);
        let zero = extract_zero(metadata);
        let quantized = self.load_quantized_weight(index);
        Some((quantized as f32 - zero as f32) * scale)
    }
}
```

**Key Innovation**: Readers never block, atomic commit guarantees consistency.

**Performance**: <10% reader overhead with concurrent updates.

**File**: `src/primitives/quant_adaptive.rs`

---

### Pattern 4: SIMD Batch Quantization

**Problem**: Scalar quantization wastes SIMD units (87.5% on AVX2).

**Solution**: Vectorized quantization using `portable_simd`.

**Implementation**:

```rust
#![feature(portable_simd)]
use std::simd::*;

pub fn quantize_simd_avx2(values: &[f32], output: &mut [u8]) {
    const SIMD_WIDTH: usize = 8;  // AVX2: 8×f32

    for chunk in values.chunks_exact(SIMD_WIDTH) {
        // Load 8 f32 into SIMD register
        let vec = f32x8::from_slice(chunk);

        // Parallel min/max reduction
        let min = vec.reduce_min();
        let max = vec.reduce_max();
        let scale = (max - min) / 15.0;

        // Parallel quantization
        let scaled = (vec - f32x8::splat(min)) / f32x8::splat(scale);
        let quantized = scaled.round().to_int();

        // Pack and store
        store_4bit(quantized, output);
    }
}
```

**Key Innovation**: Cross-platform SIMD (AVX2/AVX-512/NEON/fallback).

**Performance**: 4-8× throughput improvement.

**Feature**: `portable_simd` (nightly required)

---

## Novel Quantization Requirements

### Mandatory Compile-Time Verification

All capsules MUST use verification macros from `atomic_capsule`:

```rust
use atomic_capsule::{verify_capsule, verify_alignment};

// Full capsule verification (alignment + size)
verify_capsule!(MicroBlockQuantCapsule, 64, 128);

// Alignment-only verification
verify_alignment!(HotWeightCapsule, 64);
```

**Purpose**: Catch alignment/size violations at compile-time (zero runtime cost).

**Enforcement**: Build fails if capsule is misaligned or wrong size.

---

### ASSUM Framework Annotations

All capsules MUST document safety assumptions:

```rust
// #ASSUME_CACHE_ALIGNED: 64-byte alignment ensures single cache line read
// #VERIFY_CACHE_ALIGNED: verify_capsule!(MicroBlockQuantCapsule, 64, 128)

// #ASSUME_SCALE_RANGE: f16 covers typical activation ranges (±65504)
// #VERIFY_SCALE_RANGE: Unit tests with extreme values

// #ASSUME_4BIT_SUFFICIENT: 16 quantization levels adequate for inference
// #VERIFY_4BIT_SUFFICIENT: MSE < 0.01 validation

// #ASSUME_TOCTOU_SAFE: Generation counter prevents torn reads
// #VERIFY_TOCTOU_SAFE: Property tests with concurrent readers
```

---

### Performance Targets (B32 Validated)

All performance claims MUST meet B32 framework standards:

```rust
// Benchmark with statistical rigor
#[bench]
fn bench_mbcq_dequantization(b: &mut Bencher) {
    let capsule = MicroBlockQuantCapsule::new();
    let mut output = vec![0.0f32; 64];

    b.iter(|| {
        black_box(capsule.dequantize(&mut output))
    });

    // Validate: Mean < 50ns (target: 35ns ± 15ns margin)
    // 95% CI, n≥1000 iterations
}
```

**Targets**:
- MBCQ dequantization: <50ns for 64 weights
- Tiered hot access: <10ns per weight
- Adaptive weight load: <12ns per weight
- Gradient compression: <100ns for 64 gradients

---

## Trait Hierarchy

### Base Trait: QuantizedCapsule

```rust
pub trait QuantizedCapsule: ComputationalCapsule {
    const BIT_WIDTH: usize;         // 1, 2, 4, 8, 16 bits
    const COMPRESSION_RATIO: f32;   // vs FP32
    const GROUP_SIZE: usize;        // Granularity

    fn quantize(&mut self, input: &[f32]) -> QuantResult<()>;
    fn dequantize(&self, output: &mut [f32]) -> QuantResult<()>;
    fn generation(&self) -> u32;
}
```

### Static Quantization

```rust
pub trait StaticQuantizedCapsule: QuantizedCapsule {
    type ScaleType: Copy + Send + Sync;
    type ZeroPointType: Copy + Send + Sync;

    fn calibrate(data: &[f32]) -> (Self::ScaleType, Self::ZeroPointType);
    fn scale(&self) -> Self::ScaleType;
    fn zero_point(&self) -> Self::ZeroPointType;
}
```

### Adaptive Quantization

```rust
pub trait AdaptiveQuantizedCapsule: QuantizedCapsule {
    type AdaptiveParams: Copy + Send + Sync;

    const NUM_ADAPTIVE_CHANNELS: usize;

    fn adapt(&mut self, params: Self::AdaptiveParams);
    fn statistics(&self) -> (f32, f32, u32);  // min, max, count
}
```

---

## Testing Requirements (T28 Framework)

### Unit Tests (Q1-Q7)

```rust
#[test]
fn test_capsule_alignment() {
    assert_eq!(core::mem::align_of::<MicroBlockQuantCapsule>(), 64);
    assert_eq!(core::mem::size_of::<MicroBlockQuantCapsule>(), 128);
}

#[test]
fn test_quantize_dequantize_roundtrip() {
    let mut capsule = MicroBlockQuantCapsule::new();
    let input: Vec<f32> = (0..64).map(|i| i as f32 * 0.1).collect();

    capsule.quantize(&input).unwrap();

    let mut output = vec![0.0f32; 64];
    capsule.dequantize(&mut output).unwrap();

    // Validate MSE < 0.01
    let mse: f32 = input.iter().zip(output.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>() / 64.0;

    assert!(mse < 0.01, "MSE too high: {}", mse);
}
```

### Property Tests (Q8-Q14)

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_quantize_never_panics(values in prop::collection::vec(-1000.0f32..1000.0f32, 64)) {
        let mut capsule = MicroBlockQuantCapsule::new();
        let _ = capsule.quantize(&values);  // Should never panic
    }

    #[test]
    fn prop_generation_always_even_after_commit(
        values in prop::collection::vec(-100.0f32..100.0f32, 64)
    ) {
        let mut capsule = MicroBlockQuantCapsule::new();
        capsule.quantize(&values).unwrap();

        // Generation must be even after commit
        assert_eq!(capsule.generation() % 2, 0);
    }
}
```

### Concurrent Safety Tests (Q15-Q21)

```rust
#[test]
fn test_concurrent_readers() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(MicroBlockQuantCapsule::new());
    let mut handles = vec![];

    for _ in 0..8 {
        let capsule_clone = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            let mut output = vec![0.0f32; 64];
            for _ in 0..10_000 {
                let _ = capsule_clone.dequantize(&mut output);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    // No panics or data races = success
}
```

---

## Integration Patterns (I20 Framework)

### Extending Atomic Capsule Foundation

This crate is a **pure extension** (Tier 2/3 Computational Capsules):

```rust
// Foundation (Tier 1 - Atomic Capsules)
use atomic_capsule::{HotTier, WarmTier, ColdTier};

// Extension (Tier 2 - SIMD Capsules)
impl HotTier for MicroBlockQuantCapsule {}

// Extension (Tier 3 - Fixed-Point Capsules)
impl WarmTier for AdaptiveQuantCapsule {}
```

**Zero breaking changes** to foundation crate required.

---

## Development Workflow

### Before Implementation

1. **Read The Computational Capsule** - Understand one-read decision principle
2. **Read Atomic Capsule Foundation** - Understand verification macros and alignment
3. **Apply UCE33 Q33** - How do capsules transform this quantization problem?
4. **Apply ASSUM Framework** - Document safety assumptions

### During Implementation

1. **Use verification macros** - `verify_capsule!` at module level
2. **Follow commit-flip protocol** - Odd→even generation for lockfree updates
3. **Document assumptions** - `#ASSUME` and `#VERIFY` comments
4. **Write tests first** - T28 framework (unit, property, concurrent)

### After Implementation

1. **Run benchmarks** - B32 framework with 95% CI
2. **Validate accuracy** - MSE < target threshold
3. **Test concurrent safety** - Multi-threaded stress tests
4. **Document performance** - Update BENCHMARKS.md

---

## Trade Secret Notice

Some quantization algorithms in this project contain **trade secret optimizations**:
- **Micro-Block Co-Location**: Novel cache alignment strategy
- **Commit-Flip Protocol**: Lockfree quantization parameter updates
- **Tiered Cache Hierarchy**: Production-validated tier assignment policy

**Never commit to public repositories**.

---

## Quick Reference

### Verification Macros

```rust
verify_capsule!(Type, alignment, size);        // Full verification
verify_alignment!(Type, alignment);             // Alignment only
verify_size!(Type, size);                       // Size only
```

### Commit-Flip Protocol

```rust
// Writer
gen.store(odd_value, Relaxed);   // Mark in-progress
update_payload();
gen.store(even_value, Release);  // Commit

// Reader
let g = gen.load(Relaxed);
if g % 2 != 0 { return None; }   // Skip in-progress
```

### Cache Alignment

```rust
#[repr(C, align(64))]   // HotTier (L1)
#[repr(C, align(128))]  // WarmTier (L2)
#[repr(C, align(256))]  // ColdTier (L3)
```

---

## References

### Documentation

- `README.md` - Quick start and algorithm overview
- `docs/NOVEL_QUANTIZATION_ALGORITHMS.md` - Technical deep dive
- `docs/BENCHMARKS.md` - B32-validated performance results

### Source Code

- `src/primitives/quant_microblock.rs` - MBCQ implementation
- `src/primitives/quant_tiered.rs` - Tiered cache implementation
- `src/primitives/quant_adaptive.rs` - Adaptive quantization
- `src/primitives/gradient_compact.rs` - Gradient compression

### Frameworks

- UCE33: Systematic discovery methodology
- ASSUM: Safety assumption validation
- B32: Performance benchmarking standards
- I20: Integration framework
- T28: Comprehensive testing strategy

---

**Project Status**: Production-ready foundation for LLM quantization research.

**Maintainer**: Samuel (Primitives project)

**Last Updated**: 2025-10-07
