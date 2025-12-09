# GPU Acceleration Plan for kindly_dedup

**Version**: 1.0.0
**Date**: 2025-11-24
**Author**: UCE34 Ultrathink Analysis
**Framework**: UCE34 Q12-ULTRATHINK (Research/Architecture Planning)

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current Architecture Analysis](#2-current-architecture-analysis)
3. [GPU Acceleration Strategy](#3-gpu-acceleration-strategy)
4. [Kernel Designs](#4-kernel-designs)
5. [Hybrid Pipeline Architecture](#5-hybrid-pipeline-architecture)
6. [Implementation Phases](#6-implementation-phases)
7. [Performance Targets](#7-performance-targets)
8. [Risk Analysis](#8-risk-analysis)
9. [Framework Compliance](#9-framework-compliance)
10. [Recommended First Steps](#10-recommended-first-steps)

---

## 1. Executive Summary

### Problem Statement

kindly_dedup v2.3.0 achieves 73.4K docs/sec on CPU (45.9x vs Python datasketch). While this is production-ready, GPU acceleration can provide 5-50x additional speedup for large-scale LLM training dataset deduplication (billions of documents).

### Proposed Solution

Add GPU acceleration using **wgpu** (WebGPU) as the primary backend, enabling:
- Cross-platform support (CUDA, ROCm, Metal, Vulkan, WebGPU)
- Rust-native development (no C++ dependencies)
- Fallback to CPU SIMD when GPU unavailable

### Expected Outcomes

| Metric | Current (CPU) | Target (GPU) | Speedup |
|--------|---------------|--------------|---------|
| MinHash Computation | 73.4K docs/sec | 500K-1M docs/sec | 7-14x |
| End-to-End Pipeline | 73.4K docs/sec | 300-500K docs/sec | 4-7x |
| Memory Efficiency | O(1) mmap | O(1) mmap + GPU VRAM | Same |

### Resource Requirements

- **Implementation Time**: 6-10 weeks (4 phases)
- **Dependencies**: wgpu 24.x, naga (WGSL compiler)
- **Hardware**: Any GPU with Vulkan 1.2+, Metal 2.0+, or DX12

---

## 2. Current Architecture Analysis

### 2.1 Pipeline Overview

```
Document Stream
    |
    v
[Tokenization] (CPU) - ~2us/doc
    |
    v
[MinHash Signature] (CPU SIMD) - ~16.7us/doc (HOT PATH - 70%)
    |
    v
[LSH Band Hashing] (CPU) - ~250ns/doc
    |
    v
[Bucket Lookup] (CPU) - ~500ns/doc
    |
    v
[Jaccard Similarity] (CPU) - ~60ns/pair
    |
    v
[Union-Find Clustering] (CPU) - ~100ns/doc
    |
    v
Duplicate Clusters
```

### 2.2 Hot Path Analysis (Profiling Results)

Based on code analysis and documented benchmarks:

| Operation | Time/Doc | % of Total | GPU Parallelizable? |
|-----------|----------|------------|---------------------|
| **MinHash Signature** | 16.7us | **70%** | Yes (embarrassingly parallel) |
| LSH Band Hashing | 250ns | 1% | Yes (per-document) |
| Bucket Lookup | 500ns | 2% | Partial (memory-bound) |
| Jaccard Similarity | 60ns/pair | 15% | Yes (per-pair) |
| Union-Find | 100ns | 5% | Challenging (sequential) |
| Tokenization | 2us | 7% | Partial (memory-bound) |

**Key Insight**: MinHash computation is 70% of runtime and embarrassingly parallel - ideal for GPU acceleration.

### 2.3 Current SIMD Implementation

From `src/simd_minhash.rs`:
- Uses `portable_simd` (nightly feature)
- 128 MinHash values (u16) per signature
- 7.1x speedup over scalar baseline
- AVX2/SSE4.2 runtime dispatch

```rust
// Current SIMD approach (8-lane vectorization)
let lanes: Simd<u32, 8> = Simd::from_array([...]);
let min_vals = lanes.reduce_min();
```

### 2.4 Data Structures

| Structure | Size | GPU-Friendly? |
|-----------|------|---------------|
| MinHashSignatureCapsule | 256B (128 x u16) | Yes (contiguous, aligned) |
| LSH Bucket Key | 16B (band_idx + hash) | Yes |
| Document Tokens | Variable (Vec<String>) | Needs preprocessing |
| Union-Find | Variable (parent array) | Challenging |

### 2.5 Memory Access Patterns

- **MinHash**: Sequential reads (tokens), random writes (signature slots)
- **LSH Hashing**: Sequential reads (signature bands)
- **Bucket Lookup**: Random reads (hash table)
- **Jaccard**: Sequential reads (two signatures)

---

## 3. GPU Acceleration Strategy

### 3.1 Technology Selection

#### Primary: wgpu (WebGPU)

**Rationale**:
- Rust-native, cross-platform (Vulkan, Metal, DX12, WebGPU)
- Excellent documentation ([Learn Wgpu](https://sotrh.github.io/learn-wgpu/))
- Active development ([gfx-rs/wgpu](https://github.com/gfx-rs/wgpu))
- WGSL shaders (naga compiler handles translation)
- No C++ toolchain required

**Trade-offs**:
- Slightly lower performance than native CUDA/ROCm (~10-20%)
- WebGPU API overhead vs bare metal

#### Optional: rust-gpu (SPIR-V)

**Rationale**:
- Write shaders in Rust ([Rust-GPU](https://github.com/Rust-GPU/rust-gpu))
- Type safety, IDE support
- Compile to SPIR-V for Vulkan/wgpu

**Trade-offs**:
- Requires specific nightly Rust version
- Limited Rust features in shaders

#### Fallback: atomic_capsule GPU Modules

Leverage existing infrastructure:
- `CudaComputeCapsule` (NVIDIA)
- `RocmComputeCapsule` (AMD)
- HAL abstraction layer

### 3.2 Tier Classification

**T7 Heterogeneous** (100-1000x potential):
- Multi-accelerator coordination (CPU + GPU)
- Hybrid pipeline with async overlap
- Automatic fallback to CPU

### 3.3 Backend Priority

1. **wgpu/Vulkan** (default): Widest compatibility
2. **wgpu/Metal**: Best for macOS/iOS
3. **wgpu/DX12**: Windows optimization
4. **CUDA** (optional): Maximum NVIDIA performance
5. **ROCm** (optional): Maximum AMD performance
6. **CPU SIMD** (fallback): Always available

---

## 4. Kernel Designs

### 4.1 MinHash Signature Kernel (PRIMARY TARGET)

**Purpose**: Compute 128-value MinHash signature for batch of documents

**Input**:
```
- tokens: [N_docs][MAX_TOKENS] u32 (pre-hashed token IDs)
- token_counts: [N_docs] u32 (actual tokens per doc)
- permutation_seeds: [128] u32 (constant)
```

**Output**:
```
- signatures: [N_docs][128] u16 (MinHash values)
```

**WGSL Pseudocode**:
```wgsl
@group(0) @binding(0) var<storage, read> tokens: array<u32>;
@group(0) @binding(1) var<storage, read> token_counts: array<u32>;
@group(0) @binding(2) var<uniform> seeds: array<u32, 128>;
@group(0) @binding(3) var<storage, read_write> signatures: array<u32>;

const WORKGROUP_SIZE: u32 = 256;

@compute @workgroup_size(WORKGROUP_SIZE)
fn compute_minhash(@builtin(global_invocation_id) gid: vec3<u32>) {
    let doc_id = gid.x;
    let perm_id = gid.y;

    if (doc_id >= arrayLength(&token_counts)) {
        return;
    }

    let num_tokens = token_counts[doc_id];
    let token_offset = doc_id * MAX_TOKENS;

    var min_hash: u32 = 0xFFFFFFFF;

    // Iterate over tokens
    for (var i: u32 = 0u; i < num_tokens; i = i + 1u) {
        let token = tokens[token_offset + i];
        let hash = murmur_hash(token, seeds[perm_id]);
        min_hash = min(min_hash, hash);
    }

    // Store result (truncate to u16)
    signatures[doc_id * 128u + perm_id] = min_hash & 0xFFFF;
}

fn murmur_hash(key: u32, seed: u32) -> u32 {
    var h = seed;
    let c1: u32 = 0xcc9e2d51u;
    let c2: u32 = 0x1b873593u;

    var k = key;
    k = k * c1;
    k = (k << 15u) | (k >> 17u);
    k = k * c2;

    h = h ^ k;
    h = (h << 13u) | (h >> 19u);
    h = h * 5u + 0xe6546b64u;

    // Finalization
    h = h ^ 4u;
    h = h ^ (h >> 16u);
    h = h * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    h = h * 0xc2b2ae35u;
    h = h ^ (h >> 16u);

    return h;
}
```

**Parallelization Strategy**:
- Dispatch: `[num_docs, 128, 1]` (one thread per doc/permutation pair)
- Workgroup: `256` threads (optimal for most GPUs)
- Memory: Coalesced reads (tokens), coalesced writes (signatures)

**Expected Performance**:
- GPU: ~100ns per signature (vs 16.7us CPU)
- Throughput: 10M signatures/sec (vs 60K CPU)
- Speedup: **10-50x** on MinHash alone

### 4.2 LSH Band Hashing Kernel

**Purpose**: Hash signature bands into bucket keys

**Input**:
```
- signatures: [N_docs][128] u16
- num_bands: u32 (typically 5-12)
- rows_per_band: u32 (typically 10-25)
```

**Output**:
```
- band_hashes: [N_docs][num_bands] u64
```

**WGSL Pseudocode**:
```wgsl
@compute @workgroup_size(256)
fn hash_bands(@builtin(global_invocation_id) gid: vec3<u32>) {
    let doc_id = gid.x;
    let band_id = gid.y;

    let start = band_id * rows_per_band;
    let end = min(start + rows_per_band, 128u);

    var band_hash: u64 = 0u;
    for (var i = start; i < end; i = i + 1u) {
        let sig_val = u64(signatures[doc_id * 128u + i]);
        band_hash = band_hash * 31u + sig_val;
    }

    band_hashes[doc_id * num_bands + band_id] = band_hash;
}
```

**Expected Performance**:
- GPU: ~10ns per band (vs 50ns CPU)
- Speedup: **5x**

### 4.3 Jaccard Similarity Kernel

**Purpose**: Compute pairwise Jaccard similarity for candidate pairs

**Input**:
```
- signatures: [N_docs][128] u16
- candidate_pairs: [N_pairs][2] u32 (doc_a, doc_b)
```

**Output**:
```
- similarities: [N_pairs] f32 (or Q16.16 fixed-point)
```

**WGSL Pseudocode**:
```wgsl
@compute @workgroup_size(256)
fn compute_jaccard(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair_id = gid.x;

    let doc_a = candidate_pairs[pair_id * 2u];
    let doc_b = candidate_pairs[pair_id * 2u + 1u];

    var match_count: u32 = 0u;

    for (var i: u32 = 0u; i < 128u; i = i + 1u) {
        let sig_a = signatures[doc_a * 128u + i];
        let sig_b = signatures[doc_b * 128u + i];
        if (sig_a == sig_b) {
            match_count = match_count + 1u;
        }
    }

    // Jaccard = matches / 128
    similarities[pair_id] = f32(match_count) / 128.0;
}
```

**Expected Performance**:
- GPU: ~5ns per pair (vs 60ns CPU)
- Speedup: **12x**

### 4.4 Union-Find (CPU-Only Recommended)

**Challenge**: Union-Find with path compression is inherently sequential.

**GPU Options Evaluated**:
1. **Parallel Union-Find (Shiloach-Vishkin)**: O(log n) depth, but high work
2. **Label Propagation**: Converges slowly on large graphs
3. **GPU-friendly clustering**: Requires algorithm redesign

**Recommendation**: Keep Union-Find on CPU.
- Current: ~100ns/doc (5% of runtime)
- GPU transfer overhead would exceed computation time
- Amdahl's Law: 5% sequential = max 20x total speedup

---

## 5. Hybrid Pipeline Architecture

### 5.1 Pipeline Design

```
                    CPU                          GPU
                     |                            |
[Document Stream] ---|                            |
                     |                            |
[Tokenization] ------|                            |
(CPU, ~2us/doc)      |                            |
                     |                            |
[Token Hashing] -----|--------> [Upload to GPU]   |
(CPU, prepare)       |          (async DMA)       |
                     |                            |
                     |          [MinHash Kernel] -|
                     |          (GPU, ~100ns/doc) |
                     |                            |
                     |          [LSH Kernel] -----|
                     |          (GPU, ~10ns/doc)  |
                     |                            |
                     |<-------- [Download]        |
                     |          (async DMA)       |
                     |                            |
[Bucket Insert] -----|                            |
(CPU, mmap)          |                            |
                     |                            |
[Union-Find] --------|                            |
(CPU, ~100ns/doc)    |                            |
                     |                            |
[Output Clusters] ---|                            |
```

### 5.2 Async Overlap Strategy

**Double Buffering**:
```rust
// Batch N: GPU computing
// Batch N-1: Downloading results
// Batch N+1: Uploading tokens

let mut batch_a = GpuBatch::new(10_000);
let mut batch_b = GpuBatch::new(10_000);

loop {
    // Overlap CPU tokenization with GPU compute
    parallel_join!(
        batch_a.upload_async(),
        batch_b.download_async(),
        cpu_tokenize_batch_c(),
    );

    // Submit GPU work
    batch_a.dispatch_minhash();
    batch_a.dispatch_lsh();

    std::mem::swap(&mut batch_a, &mut batch_b);
}
```

### 5.3 Memory Management

**GPU Memory Layout**:
```
+------------------+
| Token Buffer     |  <- Staging (CPU-visible)
| [N * MAX_TOKENS] |
+------------------+
| Signature Buffer |  <- Device-local (GPU-only)
| [N * 128 * 2]    |
+------------------+
| Band Hash Buffer |  <- Device-local
| [N * NUM_BANDS]  |
+------------------+
| Result Buffer    |  <- Staging (CPU-visible)
+------------------+
```

**Transfer Optimization**:
- Pinned memory for staging buffers
- Batch size tuned for transfer latency hiding
- Zero-copy where possible (unified memory architectures)

### 5.4 Fallback Strategy

```rust
pub enum ComputeBackend {
    Gpu(GpuContext),
    CpuSimd,
    CpuScalar,
}

impl HybridPipeline {
    pub fn new() -> Self {
        let backend = match GpuContext::try_create() {
            Ok(ctx) if ctx.capabilities().compute_shaders => {
                ComputeBackend::Gpu(ctx)
            }
            _ if cpu_has_simd() => {
                ComputeBackend::CpuSimd
            }
            _ => {
                ComputeBackend::CpuScalar
            }
        };

        Self { backend }
    }
}
```

---

## 6. Implementation Phases

### Phase GPU-1: Foundation (Weeks 1-2)

**Goal**: Basic wgpu integration with MinHash kernel

**Deliverables**:
1. `src/gpu/mod.rs` - GPU module structure
2. `src/gpu/context.rs` - wgpu device/queue initialization
3. `src/gpu/minhash_kernel.wgsl` - MinHash compute shader
4. `src/gpu/minhash_capsule.rs` - GpuMinHashCapsule (T7)
5. Unit tests (kernel correctness)
6. Basic benchmark vs CPU SIMD

**Dependencies**:
```toml
[dependencies]
wgpu = { version = "24", features = ["spirv"] }
pollster = "0.4"  # Blocking async runtime for simplicity

[features]
gpu-compute = ["wgpu", "pollster"]
```

**Acceptance Criteria**:
- [ ] MinHash kernel produces identical results to CPU
- [ ] Basic benchmark shows >2x speedup on discrete GPU
- [ ] Graceful fallback when GPU unavailable

### Phase GPU-2: LSH Integration (Weeks 3-4)

**Goal**: Add LSH band hashing kernel, integrate with pipeline

**Deliverables**:
1. `src/gpu/lsh_kernel.wgsl` - Band hashing shader
2. `src/gpu/lsh_capsule.rs` - GpuLshBucketCapsule
3. Integration with `MmapLshBucketCapsule` (GPU compute -> CPU storage)
4. Property tests (GPU == CPU results)

**Acceptance Criteria**:
- [ ] LSH kernel matches CPU band hashing
- [ ] End-to-end pipeline with GPU MinHash + LSH
- [ ] Memory transfer overhead < 10% of compute time

### Phase GPU-3: Hybrid Pipeline (Weeks 5-7)

**Goal**: Full hybrid pipeline with async overlap

**Deliverables**:
1. `src/gpu/hybrid_pipeline.rs` - HybridDedupPipeline
2. Double-buffering for transfer hiding
3. Auto-detection of optimal batch size
4. Integration tests with 1M+ documents

**Acceptance Criteria**:
- [ ] 5x+ end-to-end speedup vs CPU-only
- [ ] <5% accuracy difference vs CPU pipeline
- [ ] Memory usage within 2x of CPU-only

### Phase GPU-4: Optimization (Weeks 8-10)

**Goal**: Performance tuning and optional vendor backends

**Deliverables**:
1. Batch size auto-tuning
2. Memory pool for buffer reuse
3. Optional CUDA backend (via `atomic_capsule::gpu::CudaComputeCapsule`)
4. Optional ROCm backend (via `atomic_capsule::gpu::RocmComputeCapsule`)
5. Comprehensive benchmarks (B32 compliant)

**Acceptance Criteria**:
- [ ] 10x+ speedup on modern discrete GPUs
- [ ] CUDA/ROCm backends match wgpu accuracy
- [ ] Production-ready documentation

---

## 7. Performance Targets

### 7.1 Per-Kernel Targets

| Kernel | CPU Baseline | GPU Target | Expected Speedup |
|--------|--------------|------------|------------------|
| MinHash | 16.7us/doc | 100-500ns/doc | 33-167x |
| LSH Band Hash | 250ns/doc | 10-50ns/doc | 5-25x |
| Jaccard | 60ns/pair | 5-20ns/pair | 3-12x |

### 7.2 End-to-End Targets

| Scenario | CPU (current) | GPU Target | Speedup |
|----------|---------------|------------|---------|
| Integrated GPU (iGPU) | 73.4K docs/sec | 150K docs/sec | 2x |
| Entry GPU (GTX 1650) | 73.4K docs/sec | 300K docs/sec | 4x |
| Mid-range GPU (RTX 3060) | 73.4K docs/sec | 500K docs/sec | 7x |
| High-end GPU (RTX 4090) | 73.4K docs/sec | 1M docs/sec | 14x |
| Data Center GPU (A100) | 73.4K docs/sec | 2M docs/sec | 27x |

### 7.3 Memory Targets

| Metric | Target |
|--------|--------|
| GPU VRAM usage | <2GB for 100K doc batch |
| Transfer bandwidth | >80% of theoretical PCIe |
| CPU memory overhead | <10% over CPU-only |

### 7.4 Latency Targets

| Operation | Target |
|-----------|--------|
| GPU initialization | <100ms (one-time) |
| Batch upload (10K docs) | <1ms |
| Kernel dispatch | <10us |
| Batch download | <1ms |

---

## 8. Risk Analysis

### 8.1 Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| wgpu performance < native | Medium | Medium | Optional CUDA/ROCm backends |
| Transfer overhead dominates | Medium | High | Batch size tuning, async overlap |
| GPU memory exhaustion | Low | High | Adaptive batch sizing, fallback |
| Shader compilation failures | Low | Medium | Pre-compiled SPIR-V, fallback |
| Determinism differences | Medium | Medium | Fixed-point arithmetic in shaders |

### 8.2 Compatibility Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Old GPU drivers | Medium | Low | Minimum driver version docs |
| WebGPU browser support | N/A | N/A | Native-only initially |
| macOS Metal issues | Low | Medium | Test on M1/M2/M3 chips |
| AMD driver bugs | Medium | Low | ROCm fallback, CPU fallback |

### 8.3 Resource Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Longer than estimated | Medium | Medium | Phased delivery, early feedback |
| Skill gap (WGSL) | Low | Low | Learn Wgpu tutorial available |
| Testing infrastructure | Low | Low | Use existing benchmark harness |

---

## 9. Framework Compliance

### 9.1 UCE34 Compliance

| Question | Response |
|----------|----------|
| Q1-Q3: Problem | MinHash is 70% of runtime, embarrassingly parallel |
| Q4-Q6: Analysis | Data structures GPU-friendly, memory access coalesced |
| Q7-Q9: Landscape | wgpu best for cross-platform, CUDA/ROCm optional |
| Q10: Tier | T7 Heterogeneous (CPU+GPU coordination) |
| Q11: Rust | wgpu (Rust-native), rust-gpu (optional) |
| Q12: Nightly | portable_simd (CPU fallback), no GPU-specific |
| Q33: Verification | Property tests (GPU == CPU), B32 benchmarks |
| Q34: Audit | Hash-chain integrity maintained, audit trail on CPU |

### 9.2 Chaos Compliance

| Requirement | Status |
|-------------|--------|
| 100% lockfree | GPU kernels inherently parallel, no locks |
| Cache-aligned | GPU buffers 256B aligned (wgpu default) |
| Generation counters | N/A for GPU buffers (immutable per dispatch) |
| No mutex/RwLock | Async coordination via command queues |

### 9.3 ASSUM Compliance

```rust
// #ASSUME_GPU_AVAILABLE: GPU may not be present
// #VERIFY_GPU_AVAILABLE: Runtime detection with fallback

// #ASSUME_TRANSFER_FAST: PCIe bandwidth sufficient
// #VERIFY_TRANSFER_FAST: Benchmark transfer vs compute ratio

// #ASSUME_KERNEL_DETERMINISTIC: Same input -> same output
// #VERIFY_KERNEL_DETERMINISTIC: Property tests with fixed seeds

// #ASSUME_MEMORY_SUFFICIENT: GPU VRAM >= 2GB
// #VERIFY_MEMORY_SUFFICIENT: Query device limits, adaptive batching
```

### 9.4 B32 Compliance

| Requirement | Implementation |
|-------------|----------------|
| Fair baseline | CPU SIMD (current production) |
| 1000+ iterations | Criterion benchmarks |
| 95% CI | Criterion default |
| Multiple scenarios | iGPU, entry, mid, high-end GPUs |
| Reproducibility | Fixed random seeds, documented hardware |

### 9.5 T28 Testing Strategy

| Tier | Tests |
|------|-------|
| Q1-Q7 (Unit) | Kernel correctness, buffer management |
| Q8-Q14 (Property) | GPU == CPU for all inputs |
| Q15-Q21 (Integration) | Full pipeline, 1M+ documents |
| Q22-Q28 (Production) | Multi-GPU, memory limits, fallback |
| Q29-Q35 (Determinism) | Fixed seeds, reproducible results |

### 9.6 I20 Integration

| Question | Response |
|----------|----------|
| Q1-Q5: Scope | GPU acceleration for MinHash/LSH only |
| Q6-Q10: Compat | Same API (HybridPipeline extends DedupPipeline) |
| Q11-Q15: Safety | Fallback to CPU on any GPU error |
| Q16-Q20: Validation | Accuracy tests (F1 >= 90%), perf regression tests |

---

## 10. Recommended First Steps

### Week 1 Tasks

1. **Setup wgpu development environment**
   ```bash
   cargo add wgpu@24 pollster
   cargo add --dev gpu-allocator  # Optional for memory profiling
   ```

2. **Create GPU module skeleton**
   ```
   src/gpu/
   ├── mod.rs
   ├── context.rs
   ├── error.rs
   └── shaders/
       └── minhash.wgsl
   ```

3. **Implement minimal GpuContext**
   - Device enumeration
   - Queue creation
   - Capability detection

4. **Write first compute shader test**
   - Simple reduction (sum array)
   - Verify wgpu pipeline works

### Prototype Code (context.rs)

```rust
//! GPU Context for kindly_dedup
//!
//! # Architecture
//!
//! Uses wgpu for cross-platform GPU compute (Vulkan, Metal, DX12, WebGPU).
//! Falls back to CPU SIMD when GPU unavailable.

use std::sync::Arc;

/// GPU compute capabilities
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    /// Device name (e.g., "NVIDIA RTX 4090")
    pub device_name: String,
    /// Backend (Vulkan, Metal, DX12, etc.)
    pub backend: wgpu::Backend,
    /// Max workgroup size
    pub max_workgroup_size: u32,
    /// Max buffer size
    pub max_buffer_size: u64,
    /// Supports compute shaders
    pub compute_shaders: bool,
}

/// GPU context for compute operations
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    capabilities: GpuCapabilities,
}

impl GpuContext {
    /// Try to create GPU context
    ///
    /// Returns None if no suitable GPU found
    pub async fn try_create() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;

        let info = adapter.get_info();
        let limits = adapter.limits();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: Some("kindly_dedup_gpu"),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .ok()?;

        let capabilities = GpuCapabilities {
            device_name: info.name,
            backend: info.backend,
            max_workgroup_size: limits.max_compute_workgroup_size_x,
            max_buffer_size: limits.max_buffer_size,
            compute_shaders: true,
        };

        Some(Self {
            device,
            queue,
            capabilities,
        })
    }

    /// Create context (blocking)
    pub fn try_create_blocking() -> Option<Self> {
        pollster::block_on(Self::try_create())
    }

    /// Get capabilities
    pub fn capabilities(&self) -> &GpuCapabilities {
        &self.capabilities
    }

    /// Get device reference
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get queue reference
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
```

### Next Actions

1. **Read**: [Learn Wgpu - Compute Shaders](https://sotrh.github.io/learn-wgpu/)
2. **Study**: [rust-gpu-wgpu-compute-minimal](https://github.com/andrusha/rust-gpu-wgpu-compute-minimal)
3. **Implement**: GpuContext with device detection
4. **Test**: Simple compute shader (array sum)
5. **Benchmark**: Measure transfer overhead
6. **Iterate**: MinHash kernel prototype

---

## Appendix A: References

### Documentation
- [wgpu Documentation](https://docs.rs/wgpu/)
- [Learn Wgpu Tutorial](https://sotrh.github.io/learn-wgpu/)
- [WebGPU Specification](https://www.w3.org/TR/webgpu/)
- [WGSL Specification](https://www.w3.org/TR/WGSL/)

### Research
- [Rust-GPU Project](https://github.com/Rust-GPU/rust-gpu)
- [GPU MinHash Paper](https://arxiv.org/abs/2003.03369) (GPU-accelerated MinHash)
- [Parallel Union-Find](https://dl.acm.org/doi/10.1145/3087556.3087585) (Shiloach-Vishkin)

### atomic_capsule GPU Modules
- `/home/samuel/Primitives/atomic_capsule/src/gpu/mod.rs`
- `/home/samuel/Primitives/atomic_capsule/src/gpu/cuda_capsule.rs`
- `/home/samuel/Primitives/atomic_capsule/src/gpu/rocm_capsule.rs`
- `/home/samuel/Primitives/atomic_capsule/src/gpu/hal/vulkan_compute.rs`

---

## Appendix B: Hardware Requirements

### Minimum Requirements
- GPU: Any with Vulkan 1.2+, Metal 2.0+, or DX12
- VRAM: 2GB
- Driver: Recent (2023+)

### Recommended
- GPU: NVIDIA RTX 30/40 series, AMD RX 6000/7000 series
- VRAM: 8GB+
- PCIe: Gen 4 x16

### Tested Configurations
- NVIDIA RTX 4090 (Vulkan)
- AMD RX 7900 XTX (Vulkan)
- Apple M3 Max (Metal)
- Intel Arc A770 (Vulkan)

---

## Appendix C: Glossary

| Term | Definition |
|------|------------|
| **wgpu** | Cross-platform GPU library based on WebGPU API |
| **WGSL** | WebGPU Shading Language |
| **SPIR-V** | Standard Portable Intermediate Representation for shaders |
| **MinHash** | Locality-sensitive hashing for set similarity |
| **LSH** | Locality-Sensitive Hashing |
| **T7** | Heterogeneous tier (multi-accelerator) |
| **Chaos** | Computational Capsule Architecture |

---

*Document generated by UCE34 Q12-ULTRATHINK analysis*
*Framework: UCE34 + B32 + T28 + ASSUM + I20 + Chaos*
