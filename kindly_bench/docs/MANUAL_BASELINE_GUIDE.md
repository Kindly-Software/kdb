# Manual Baseline Guide for T7-T11 Specialized Tiers

**Version**: 1.0.0
**Framework**: kindly_bench Phase 3
**Purpose**: Guide for writing fair manual baselines for specialized tiers

---

## Overview

Phase 3 tiers (T7-T11) require **manual baseline implementation** because automatic generation would produce unfair strawman comparisons. This guide provides strategies, examples, and checklists for writing fair baselines.

### Why Manual Baselines?

**Automatic baselines work for T1-T6** because transformations are straightforward:
- T1 Atomic → RwLock (replace atomic ops with locks)
- T2 SIMD → Scalar (replace vector ops with loops)
- T3 Fixed-Point → F64 (replace Q16.16 with f64)

**Manual baselines required for T7-T11** because transformations are domain-specific:
- T7 GPU → CPU (requires optimized CPU library, not naive loops)
- T8 Distributed → Single-node (requires multi-threaded code, not single-threaded)
- T10 Approximate → Exact (requires optimized exact algorithm, not brute force)
- T11 Quantum → Classical (requires best-known classical algorithm)

---

## T7 Heterogeneous (GPU) - Manual Baseline

### Baseline Strategy

| Component | Implementation |
|-----------|---------------|
| **Optimized** | GPU kernel execution (CUDA, Vulkan, compute shaders) |
| **Baseline** | CPU-only version (YOU provide this) |
| **Expected Speedup** | 15-20× (EXCEPTIONAL), 20-100× (BREAKTHROUGH) |

### How to Write Fair CPU Baseline

#### Step 1: Identify GPU Kernel Operations

```cuda
__global__ void vec_add(float* a, float* b, float* c, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) c[i] = a[i] + b[i];
}
```

#### Step 2: Write Equivalent CPU Code (Optimized!)

```rust
fn vec_add_cpu(a: &[f32], b: &[f32]) -> Vec<f32> {
    // GOOD: SIMD-optimized CPU code (compiler auto-vectorizes)
    a.iter().zip(b).map(|(x, y)| x + y).collect()

    // BAD (strawman): Naive loop
    // let mut c = vec![0.0; a.len()];
    // for i in 0..a.len() { c[i] = a[i] + b[i]; }
}
```

#### Step 3: Use Well-Optimized Libraries

| Operation | Optimized Library | Strawman (AVOID) |
|-----------|-------------------|------------------|
| Matrix multiplication | OpenBLAS, Intel MKL, cuBLAS | Naive O(n³) loops |
| Convolutions | NNPACK, OneDNN | Naive nested loops |
| Reductions | CPU SIMD intrinsics | Sequential sum |
| FFT | FFTW | Naive DFT |

#### Step 4: Benchmark Both Implementations

```rust
use kindly_bench::*;

#[cfg(feature = "gpu")]
let config = BenchmarkConfig::builder()
    .tier(Tier::T7Heterogeneous)
    .gpu_timer(GpuTimer::cuda())
    .baseline_manual(Box::new(|| vec_add_cpu(&a, &b)))
    .build();
```

### Fair Baseline Checklist

- ✓ Uses well-optimized CPU library (OpenBLAS, MKL, FFTW)
- ✓ Same algorithm as GPU version (not naive implementation)
- ✓ CPU SIMD intrinsics where applicable
- ✓ Multi-threaded CPU code (if GPU uses massive parallelism)
- ✗ Naive single-threaded loop (STRAWMAN!)

---

## T8 Network - Manual Baseline

### Baseline Strategy

| Component | Implementation |
|-----------|---------------|
| **Optimized** | Distributed coordination (multiple nodes) |
| **Baseline** | Single-node version (YOU provide this) |
| **Expected Speedup** | 2-3× (TYPICAL), 3-10× (EXCEPTIONAL), 10-50× (BREAKTHROUGH) |

### How to Write Fair Single-Node Baseline

#### Step 1: Identify Distributed Operations

```rust
// Distributed (optimized, 8 nodes)
let cluster = NetworkCluster::new(8);
let model = DistributedModel::shard_across(cluster);
model.train_epoch(data);  // Pipeline parallelism
```

#### Step 2: Write Equivalent Single-Node Multi-Threaded Code

```rust
// Single-node baseline (YOU write this)
fn train_single_node(model: &Model, data: &Data) {
    // Multi-threaded training on single GPU/CPU
    // Use data parallelism (NOT naive single-threaded!)
    rayon::scope(|s| {
        for batch in data.batches(rayon::current_num_threads()) {
            s.spawn(|_| model.train_batch(batch));
        }
    });
}
```

#### Step 3: Benchmark Both Implementations

```rust
use kindly_bench::*;

#[cfg(feature = "network")]
let config = BenchmarkConfig::builder()
    .tier(Tier::T8Network)
    .instant_timer()  // Wall-clock timing
    .baseline_manual(Box::new(|| train_single_node(&model, &data)))
    .build();
```

### Fair Baseline Checklist

- ✓ Multi-threaded single-node code (rayon, std::thread)
- ✓ Same algorithm as distributed version
- ✓ Optimized data parallelism
- ✓ Realistic dataset (not toy problem)
- ✗ Single-threaded code (STRAWMAN!)

---

## T9 Persistent - Auto-Generated Baseline

### Baseline Strategy

| Component | Implementation |
|-----------|---------------|
| **Optimized** | Memory-mapped atomic persistence (durable, ACID) |
| **Baseline** | In-memory atomics (auto-generated, no durability) |
| **Expected Overhead** | 5-50% SLOWER (measuring cost of ACID guarantees) |

### Auto-Generation Process

Framework automatically generates baseline by:
1. Removing `mmap` file backing
2. Replacing `PersistentCapsule` with `AtomicU64`
3. Keeping same operations

### Example

```rust
// Optimized (T9 Persistent)
let capsule = PersistentCapsule::open_or_create("state.mmap")?;
capsule.update(new_value);  // Durable write

// Auto-generated baseline (In-memory)
let capsule = AtomicU64::new(0);
capsule.store(new_value, Ordering::Release);  // No durability
```

### Expected Results

**NOTE**: Persistence is typically SLOWER (measuring cost of durability).

- **5-15% overhead**: Write-through cache
- **20-50% overhead**: fsync per operation
- **100%+ overhead**: Small random writes

---

## T10 Probabilistic - Manual Baseline

### Baseline Strategy

| Component | Implementation |
|-----------|---------------|
| **Optimized** | Approximate algorithms (MinHash, LSH, HyperLogLog) |
| **Baseline** | Exact algorithms (YOU provide this) |
| **Expected Speedup** | 100-1000× (BREAKTHROUGH) |

### How to Write Fair Exact Baseline

#### Step 1: Identify Approximate Algorithm

```rust
// Approximate (optimized, T10 Probabilistic)
let signatures = documents.iter()
    .map(|doc| MinHashSignatureCapsule::from_document(doc))
    .collect();
let duplicates = lsh_find_duplicates(signatures, threshold = 0.85);
// Accuracy: 90-99% recall, Speed: 60K docs/sec (38× speedup)
```

#### Step 2: Write Equivalent Exact Algorithm (Optimized!)

```rust
// Exact baseline (YOU write this)
fn exact_jaccard_deduplication(documents: &[Document]) -> Vec<(usize, usize)> {
    // All-pairs Jaccard similarity (O(n²))
    // Use optimized set intersection (NOT naive nested loops!)
    let mut duplicates = Vec::new();
    for i in 0..documents.len() {
        for j in (i+1)..documents.len() {
            let jaccard = compute_jaccard_optimized(&documents[i], &documents[j]);
            if jaccard >= 0.85 {
                duplicates.push((i, j));
            }
        }
    }
    duplicates
}
```

#### Step 3: Benchmark Both + Report Accuracy

```rust
use kindly_bench::*;

let config = BenchmarkConfig::builder()
    .tier(Tier::T10Probabilistic)
    .baseline_manual(Box::new(|| exact_jaccard_deduplication(&documents)))
    .accuracy_metrics(recall = 0.95, precision = 0.92, f1 = 0.93)
    .build();
```

### Fair Baseline Checklist

- ✓ Uses optimized exact algorithm (not naive O(n³))
- ✓ Same problem definition (e.g., Jaccard threshold)
- ✓ Optimized set operations (intersection, union)
- ✓ Reports accuracy metrics (recall, precision, F1)
- ✗ Naive nested loops (STRAWMAN!)

---

## T11 QuantumHybrid - Manual Baseline

### Baseline Strategy

| Component | Implementation |
|-----------|---------------|
| **Optimized** | Quantum/neuromorphic algorithms |
| **Baseline** | Classical-only algorithms (YOU provide this) |
| **Expected Speedup** | 10-16,667× (BREAKTHROUGH) |

### How to Write Fair Classical Baseline

#### Step 1: Identify Quantum Algorithm

```rust
// Quantum/Neuromorphic (optimized, T11 QuantumHybrid)
let update = FunctionalEncryptedUpdate::new(bytecode_patch);  // 600 bytes
os_kernel.apply_encrypted_update(update)?;  // 10ms downtime
// Zero-downtime kernel patching via neuromorphic coordination
```

#### Step 2: Use Best-Known Classical Algorithm

```rust
// Classical baseline (YOU write this)
fn classical_kernel_update() -> Result<(), Error> {
    // Download full kernel image (10GB)
    download_kernel_image("https://cdn.ubuntu.com/kernel.img")?;

    // Reboot system (5-10 min downtime)
    reboot_system()?;

    Ok(())
}
```

#### Step 3: Benchmark Both Implementations

```rust
use kindly_bench::*;

#[cfg(feature = "quantum")]
let config = BenchmarkConfig::builder()
    .tier(Tier::T11QuantumHybrid)
    .quantum_timer(QuantumTimer::simulated())
    .baseline_manual(Box::new(|| classical_kernel_update()))
    .build();
```

### Expected Results

- **BREAKTHROUGH**: 10-16,667× speedup
  - Data transfer: 10GB → 600 bytes (16,667×)
  - Downtime: 5 min → 10ms (30,000×)

### Fair Baseline Checklist

- ✓ Uses best-known classical algorithm
- ✓ Same problem definition
- ✓ Realistic scenario (not toy problem)
- ✓ Reports multiple speedup dimensions (data, time, etc.)
- ✗ Naive classical approach (STRAWMAN!)

---

## Summary

### Auto-Generated Baselines (T9 Only)

| Tier | Optimized | Baseline | Framework Action |
|------|-----------|----------|------------------|
| T9 Persistent | Mmap atomics | In-memory atomics | Auto-generates |

### Manual Baselines (T7-T8, T10-T11)

| Tier | Optimized | Baseline | Your Action |
|------|-----------|----------|-------------|
| T7 Heterogeneous | GPU kernel | CPU library (OpenBLAS, MKL) | **YOU implement** |
| T8 Network | Distributed | Single-node multi-threaded | **YOU implement** |
| T10 Probabilistic | Approximate | Exact (optimized) | **YOU implement** |
| T11 QuantumHybrid | Quantum | Classical (best-known) | **YOU implement** |

### General Principles

1. **Fair baselines use optimized libraries**, not naive implementations
2. **Same algorithm/problem definition** (not different approach)
3. **Report accuracy trade-offs** for T10 Probabilistic
4. **Document speedup dimensions** for T11 QuantumHybrid (data, time, etc.)
5. **B32 validation enforced** (95% CI, hardware checks, fair baseline detection)

---

## Framework Support

kindly_bench provides:
- **Tier-specific guides** (this document)
- **Manual baseline API** (`baseline_manual(Box::new(|| your_baseline()))`)
- **B32 validation** (strawman detection via >10× suspicious threshold)
- **XML output** (machine-readable results for CI/CD)

---

**Next Steps**: See `examples/phase3/` for complete working examples of each tier.
