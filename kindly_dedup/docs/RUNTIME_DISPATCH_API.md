# Runtime SIMD Dispatch API Reference

**Version**: 1.0
**Date**: 2025-11-02
**Module**: `atomic_capsule::CpuCapabilityCapsule` + `kindly_dedup::DedupPipeline`

---

## Table of Contents

1. [Overview](#overview)
2. [CpuCapabilityCapsule API](#cpucapabilitycapsule-api)
3. [DedupPipeline API](#deduppipeline-api)
4. [Feature Flags](#feature-flags)
5. [Error Handling](#error-handling)
6. [Platform-Specific Notes](#platform-specific-notes)
7. [Code Examples](#code-examples)

---

## Overview

The Runtime SIMD Dispatch API provides CPU feature detection and automatic SIMD optimization selection at runtime. It consists of two main components:

1. **CpuCapabilityCapsule** (from `atomic_capsule`): CPU feature detection singleton
2. **DedupPipeline** (from `kindly_dedup`): Deduplication pipeline with SIMD dispatch

---

## CpuCapabilityCapsule API

**Module**: `atomic_capsule::primitives::cpu_capabilities`
**Source**: `/home/samuel/Primitives/atomic_capsule/src/primitives/cpu_capabilities.rs`

### Type Definition

```rust
#[repr(C, align(64))]
pub struct CpuCapabilityCapsule {
    avx512: AtomicBool,
    avx2: AtomicBool,
    sse42: AtomicBool,
    neon: AtomicBool,
    generation: AtomicU64,
    _padding: [u8; 48],
}
```

**Size**: 64 bytes (cache-line aligned)
**Lifetime**: `'static` (singleton pattern via `OnceLock`)
**Thread Safety**: 100% safe (immutable after initialization)

---

### Detection Methods

#### `detect() -> &'static Self`

Initialize or retrieve the CPU capabilities singleton.

**Performance**:
- First call: ~1ms (CPUID detection + initialization)
- Subsequent calls: <10ns (cached pointer dereference)

**Thread Safety**: Safe to call concurrently (OnceLock guarantees exactly-once initialization)

**Example**:
```rust
use atomic_capsule::CpuCapabilityCapsule;

let caps = CpuCapabilityCapsule::detect();
println!("Best SIMD tier: {}", caps.best_simd_tier());
```

**Platforms**:
- `x86_64`: Detects AVX-512F, AVX2, SSE4.2 via CPUID
- `aarch64`: NEON always available (ARMv8-A baseline)
- Other: All features disabled (graceful scalar fallback)

---

### Feature Query Methods

#### `has_avx512(&self) -> bool`

Check if CPU supports AVX-512F (16-lane SIMD).

**Performance**: <10ns (Relaxed atomic load)

**Supported CPUs**:
- Intel Xeon Scalable 2017+ (Skylake-SP, Cascade Lake, Ice Lake, Sapphire Rapids)
- Not available on consumer CPUs (Core i7/i9) or AMD

**Example**:
```rust
if caps.has_avx512() {
    println!("Using 16-lane AVX-512 SIMD");
    compute_avx512(data);
}
```

---

#### `has_avx2(&self) -> bool`

Check if CPU supports AVX2 (8-lane SIMD).

**Performance**: <10ns (Relaxed atomic load)

**Supported CPUs**:
- Intel: Haswell 2013+, Broadwell, Skylake, Kaby Lake, Coffee Lake, etc.
- AMD: Excavator 2015+, Ryzen 2017+, EPYC 2017+
- **Coverage**: 97%+ of desktop/server CPUs (2013+)

**Example**:
```rust
if caps.has_avx2() {
    println!("Using 8-lane AVX2 SIMD");
    compute_avx2(data);
}
```

---

#### `has_sse42(&self) -> bool`

Check if CPU supports SSE4.2 (4-lane SIMD).

**Performance**: <10ns (Relaxed atomic load)

**Supported CPUs**:
- Intel: Nehalem 2008+, Westmere, Sandy Bridge, Ivy Bridge, etc.
- AMD: Bulldozer 2011+, Piledriver, Steamroller, etc.
- **Coverage**: 99%+ of CPUs (2008+)

**Example**:
```rust
if caps.has_sse42() {
    println!("Using 4-lane SSE4.2 SIMD");
    compute_sse42(data);
}
```

---

#### `has_neon(&self) -> bool`

Check if CPU supports ARM NEON (4-lane SIMD).

**Performance**: <10ns (Relaxed atomic load)

**Supported CPUs**:
- All aarch64 CPUs (ARMv8-A architecture mandate)
- Apple M1/M2/M3, AWS Graviton, Ampere Altra
- **Coverage**: 100% of aarch64 (always `true`)

**Example**:
```rust
if caps.has_neon() {
    println!("Using 4-lane ARM NEON SIMD");
    compute_neon(data);
}
```

---

#### `best_simd_tier(&self) -> &'static str`

Get the best available SIMD instruction set.

**Performance**: ~20ns (4 atomic loads + branch prediction)

**Returns**:
- `"avx512"`: AVX-512F available (16-lane)
- `"avx2"`: AVX2 available (8-lane)
- `"sse4.2"`: SSE4.2 available (4-lane)
- `"neon"`: ARM NEON available (4-lane)
- `"scalar"`: No SIMD available (portable fallback)

**Example**:
```rust
match caps.best_simd_tier() {
    "avx512" => compute_avx512(data),
    "avx2" => compute_avx2(data),
    "sse4.2" => compute_sse42(data),
    "neon" => compute_neon(data),
    "scalar" => compute_scalar(data),
    _ => unreachable!(),
}
```

---

#### `generation(&self) -> u64`

Get generation counter (for TOCTOU prevention).

**Performance**: <10ns (Acquire atomic load)

**Returns**: Always `1` after initialization

**Use Case**: Verify singleton initialization completed

**Example**:
```rust
assert_eq!(caps.generation(), 1);
```

---

### Debug Output

#### `impl Debug for CpuCapabilityCapsule`

Pretty-print CPU capabilities.

**Example**:
```rust
let caps = CpuCapabilityCapsule::detect();
println!("{:?}", caps);

// Output:
// CpuCapabilityCapsule {
//     avx512: false,
//     avx2: true,
//     sse42: true,
//     neon: false,
//     generation: 1,
//     best_tier: "avx2"
// }
```

---

## DedupPipeline API

**Module**: `kindly_dedup`
**Source**: `/home/samuel/Primitives/kindly_dedup/src/pipeline.rs`

### Type Definition

```rust
pub struct DedupPipeline<'a> {
    signatures: Vec<Option<MinHashSignatureCapsule>>,
    bloom_filter: DedupBloomFilter,
    num_documents: usize,
    documents_added: usize,
    documents_skipped: usize,
    cpu_caps: &'a CpuCapabilityCapsule,
}
```

**Lifetime**: `'a` (borrows `CpuCapabilityCapsule` reference)
**Size**: ~16 bytes (reference + counters) + `O(n)` signature storage

---

### Construction

#### `new(num_documents: usize, cpu_caps: &'a CpuCapabilityCapsule) -> Self`

Create new deduplication pipeline with CPU capabilities reference.

**Parameters**:
- `num_documents`: Expected document capacity (for pre-allocation)
- `cpu_caps`: Reference to CPU capabilities singleton

**Performance**:
- Allocation: O(n) for signature storage (~256 bytes per document)
- Reference storage: 8 bytes (one-time)
- Total: ~1-2ms for 10K documents

**Example**:
```rust
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;

let cpu_caps = CpuCapabilityCapsule::detect();
let pipeline = DedupPipeline::new(10_000, &cpu_caps);
```

**Panics**: None

**Errors**: None (infallible constructor)

---

### Document Operations

#### `add_document(&mut self, doc_id: DocId, text: &str) -> Result<(), PipelineError>`

Add document to pipeline with automatic SIMD dispatch.

**Parameters**:
- `doc_id`: Unique document ID (must be `< num_documents`)
- `text`: Document text (UTF-8 encoded)

**Performance** (with `simd-minhash` feature):
- Bloom pre-check: <30ns (early-exit if duplicate)
- Tokenization: ~10μs (500 words)
- CPU check: <10ns (Relaxed atomic load)
- SIMD MinHash: ~1.2μs (AVX2, 6-7× faster than 8.5μs scalar)
- Total: <30ns for duplicates, ~11.2μs for new documents

**Performance** (without `simd-minhash` feature):
- Bloom pre-check: <30ns
- Tokenization: ~10μs
- Scalar MinHash: ~8.5μs
- Total: <30ns for duplicates, ~18.5μs for new documents

**Returns**:
- `Ok(())`: Document added successfully
- `Err(PipelineError::DocumentIdOutOfBounds)`: `doc_id >= num_documents`

**Example**:
```rust
pipeline.add_document(0, "The quick brown fox")?;
pipeline.add_document(1, "The quick brown fox")?; // Duplicate (skipped by Bloom)
```

**Panics**: If `doc_id >= num_documents` (bounds check)

**SIMD Dispatch** (when `simd-minhash` enabled):
```rust
if self.cpu_caps.has_avx2() || self.cpu_caps.has_sse42() {
    simd_compute_signature(&token_refs)  // 6-7× faster
} else {
    MinHashSignatureCapsule::compute_signature(&token_refs)
}
```

---

#### `find_duplicates(&self, threshold: f64) -> Result<Vec<Vec<DocId>>, PipelineError>`

Find all duplicate clusters using LSH + Jaccard similarity.

**Parameters**:
- `threshold`: Jaccard similarity threshold (0.0 to 1.0, typically 0.85)

**Performance**:
- Band hashing: <500ns per document
- Pairwise comparison: O(candidates) where candidates << n²
- Union-Find: <100μs for 10K documents
- Total: <1ms for 10K documents

**Returns**:
- `Ok(Vec<Vec<DocId>>)`: List of duplicate clusters
  - Each cluster is a `Vec<DocId>` (documents with similarity ≥ threshold)
  - Singleton clusters (single document) included
- `Err(PipelineError)`: See error handling section

**Example**:
```rust
let clusters = pipeline.find_duplicates(0.85)?;

for (i, cluster) in clusters.iter().enumerate() {
    if cluster.len() > 1 {
        println!("Cluster {}: {:?}", i, cluster);
    }
}
```

**Algorithm**:
1. LSH band-based bucketing (5 bands × 25 rows)
2. Candidate pair extraction (documents in same bucket)
3. Jaccard similarity verification (Q16.16 deterministic)
4. Union-Find clustering (O(α(n)) path compression)

**Accuracy**:
- Recall: 92-99% (LSH approximation)
- Precision: 94-100% (Jaccard verification)
- F1 Score: ≥90%

---

### Query Methods

#### `documents_added(&self) -> usize`

Get number of documents added (excluding Bloom-skipped duplicates).

**Example**:
```rust
println!("Added {} unique documents", pipeline.documents_added());
```

---

#### `documents_skipped(&self) -> usize`

Get number of documents skipped by Bloom filter (likely duplicates).

**Example**:
```rust
println!("Skipped {} duplicates", pipeline.documents_skipped());
```

---

#### `skip_rate(&self) -> f64`

Get Bloom filter skip rate (0.0 to 1.0).

**Formula**: `documents_skipped / (documents_added + documents_skipped)`

**Example**:
```rust
println!("Skip rate: {:.2}%", pipeline.skip_rate() * 100.0);
```

**Typical Values**:
- Low-duplicate corpus (<10% duplicates): 5-15% skip rate
- Medium-duplicate corpus (50% duplicates): 45-55% skip rate
- High-duplicate corpus (90% duplicates): 85-95% skip rate

---

#### `capacity(&self) -> usize`

Get total document capacity.

**Example**:
```rust
println!("Capacity: {} documents", pipeline.capacity());
```

---

## Feature Flags

### `simd-minhash`

**Purpose**: Enable SIMD-accelerated MinHash computation

**Requirements**:
- Rust nightly (for `portable_simd`)
- `#![feature(portable_simd)]` in lib.rs

**Performance**:
- AVX2 CPUs: 6-7× speedup (1.2μs vs 8.5μs scalar)
- Non-AVX2 CPUs: Automatic scalar fallback (no regression)

**Build**:
```bash
cargo +nightly build --release --features simd-minhash
```

**Runtime Behavior**:
- `has_avx2()` or `has_sse42()` → SIMD path (6-7× faster)
- Otherwise → Scalar fallback (same as without feature)

**Binary Size**: +15% (~12 MB vs ~7 MB without SIMD)

---

### Future: `simd-minhash-stable`

**Purpose**: SIMD on stable Rust (when `portable_simd` stabilizes)

**Status**: Planned for Rust 1.82+ (Q1 2025)

**Build** (future):
```bash
cargo build --release --features simd-minhash-stable
```

---

## Error Handling

### `PipelineError`

**Enum Definition**:
```rust
#[derive(Debug)]
pub enum PipelineError {
    DocumentIdOutOfBounds { doc_id: usize, capacity: usize },
}
```

---

#### `DocumentIdOutOfBounds`

**Cause**: `doc_id >= num_documents` in `add_document()`

**Example**:
```rust
let pipeline = DedupPipeline::new(10, &cpu_caps);
match pipeline.add_document(20, "text") {
    Err(PipelineError::DocumentIdOutOfBounds { doc_id, capacity }) => {
        eprintln!("Doc ID {} out of bounds (capacity: {})", doc_id, capacity);
    }
    Ok(()) => println!("Success"),
}
```

**Prevention**:
- Ensure `doc_id < num_documents` before calling `add_document()`
- Pre-allocate sufficient capacity in `DedupPipeline::new()`

---

### Error Propagation

**Pattern**: Use `?` operator for automatic error propagation

```rust
fn process_documents(
    pipeline: &mut DedupPipeline,
    documents: &[(DocId, &str)]
) -> Result<Vec<Vec<DocId>>, PipelineError> {
    for (doc_id, text) in documents {
        pipeline.add_document(*doc_id, text)?;  // Auto-propagate errors
    }
    pipeline.find_duplicates(0.85)
}
```

---

## Platform-Specific Notes

### x86_64 (Intel/AMD)

**Detection Method**: `is_x86_feature_detected!()` macro (CPUID)

**Supported Features**:
- AVX-512F: Intel Xeon Scalable 2017+ (Skylake-SP, Cascade Lake, Ice Lake)
- AVX2: Intel Haswell 2013+, AMD Excavator 2015+
- SSE4.2: Intel Nehalem 2008+, AMD Bulldozer 2011+

**Performance**:
- AVX2: 6-7× speedup (validated on Intel Core Ultra 7 155H, AMD Ryzen 9 6900HX)
- AVX-512: Not yet tested (estimated 12-14× speedup)

**Build Flags**:
```bash
# DO NOT use target-cpu=native for distribution binaries
# BAD (CPU-specific, breaks on older CPUs):
RUSTFLAGS="-C target-cpu=native" cargo build --release

# GOOD (universal binary with runtime dispatch):
cargo +nightly build --release --features simd-minhash
```

---

### aarch64 (ARM64)

**Detection Method**: Compile-time constant (NEON always available)

**Supported Features**:
- NEON: All ARMv8-A CPUs (Apple M1/M2/M3, AWS Graviton, Ampere Altra)

**Performance**:
- NEON: Not yet tested (estimated 3-4× speedup)

**Build**:
```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

**Cross-Compilation** (from x86_64):
```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

---

### Windows

**Detection Method**: Same as x86_64 Linux (CPUID via `is_x86_feature_detected!()`)

**Build**:
```bash
cargo +nightly build --release --features simd-minhash
```

**Known Issues**:
- None (same behavior as Linux)

---

### macOS

**Detection Method**:
- x86_64: CPUID via `is_x86_feature_detected!()`
- aarch64 (M1/M2/M3): NEON always available

**Build**:
```bash
cargo +nightly build --release --features simd-minhash
```

**Apple Silicon (M1/M2/M3)**:
- NEON: Always available (4-lane SIMD)
- Performance: Not yet tested

---

### WASM (WebAssembly)

**Detection Method**: All features disabled (no SIMD detection)

**Fallback**: Scalar-only code path

**Build**:
```bash
cargo build --release --target wasm32-unknown-unknown
# No --features simd-minhash (not supported)
```

**WASM SIMD** (future):
- WebAssembly SIMD proposal (128-bit SIMD)
- Not yet integrated (future work)

---

## Code Examples

### Basic Usage

```rust
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Detect CPU capabilities
    let cpu_caps = CpuCapabilityCapsule::detect();
    println!("CPU tier: {}", cpu_caps.best_simd_tier());

    // 2. Create pipeline
    let mut pipeline = DedupPipeline::new(1000, &cpu_caps);

    // 3. Add documents
    let documents = vec![
        (0, "The quick brown fox jumps over the lazy dog"),
        (1, "The quick brown fox leaps over the lazy dog"),
        (2, "Completely different document"),
    ];

    for (doc_id, text) in documents {
        pipeline.add_document(doc_id, text)?;
    }

    // 4. Find duplicates
    let clusters = pipeline.find_duplicates(0.85)?;

    for (i, cluster) in clusters.iter().enumerate() {
        if cluster.len() > 1 {
            println!("Cluster {}: {:?}", i, cluster);
        }
    }

    Ok(())
}
```

---

### Error Handling

```rust
use kindly_dedup::{DedupPipeline, PipelineError};
use atomic_capsule::CpuCapabilityCapsule;

fn process_corpus(
    documents: &[(usize, &str)]
) -> Result<Vec<Vec<usize>>, PipelineError> {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(documents.len(), &cpu_caps);

    for (doc_id, text) in documents {
        match pipeline.add_document(*doc_id, text) {
            Ok(()) => continue,
            Err(PipelineError::DocumentIdOutOfBounds { doc_id, capacity }) => {
                eprintln!("Error: Doc ID {} exceeds capacity {}", doc_id, capacity);
                return Err(PipelineError::DocumentIdOutOfBounds { doc_id: *doc_id, capacity });
            }
        }
    }

    pipeline.find_duplicates(0.85)
}
```

---

### Multi-Pipeline (Shared CPU Caps)

```rust
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;

fn main() {
    // Detect CPU capabilities once (shared across pipelines)
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Create multiple pipelines with same CPU caps
    let pipeline1 = DedupPipeline::new(10_000, &cpu_caps);
    let pipeline2 = DedupPipeline::new(50_000, &cpu_caps);
    let pipeline3 = DedupPipeline::new(100_000, &cpu_caps);

    // All pipelines use same SIMD tier (no re-detection)
}
```

---

### Conditional SIMD Logging

```rust
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;

fn main() {
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Log SIMD tier
    println!("Best SIMD tier: {}", cpu_caps.best_simd_tier());
    println!("AVX2: {}", caps.has_avx2());
    println!("AVX-512: {}", caps.has_avx512());
    println!("SSE4.2: {}", caps.has_sse42());
    println!("NEON: {}", caps.has_neon());

    let mut pipeline = DedupPipeline::new(1000, &cpu_caps);

    // Add documents with SIMD dispatch logging
    #[cfg(feature = "simd-minhash")]
    if caps.has_avx2() {
        println!("Using SIMD MinHash (6-7× speedup expected)");
    } else {
        println!("Using scalar MinHash (fallback)");
    }
}
```

---

### Performance Measurement

```rust
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;
use std::time::Instant;

fn main() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10_000, &cpu_caps);

    let documents = vec![
        (0, "Document 0 with some text"),
        (1, "Document 1 with different text"),
        // ... 10K documents
    ];

    // Measure add_document performance
    let start = Instant::now();
    for (doc_id, text) in &documents {
        pipeline.add_document(*doc_id, text).unwrap();
    }
    let add_time = start.elapsed();

    println!("Added {} docs in {:?}", documents.len(), add_time);
    println!("Throughput: {:.0} docs/sec", documents.len() as f64 / add_time.as_secs_f64());

    // Measure find_duplicates performance
    let start = Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_time = start.elapsed();

    println!("Found {} clusters in {:?}", clusters.len(), find_time);
}
```

---

### Cross-Platform Detection

```rust
use atomic_capsule::CpuCapabilityCapsule;

fn main() {
    let caps = CpuCapabilityCapsule::detect();

    #[cfg(target_arch = "x86_64")]
    {
        println!("Platform: x86_64");
        if caps.has_avx2() {
            println!("AVX2 available (8-lane)");
        } else if caps.has_sse42() {
            println!("SSE4.2 available (4-lane)");
        } else {
            println!("No SIMD available");
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        println!("Platform: aarch64");
        if caps.has_neon() {
            println!("NEON available (4-lane)");
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        println!("Platform: Other (scalar fallback)");
    }
}
```

---

**END OF API REFERENCE**

For architecture details, see `RUNTIME_DISPATCH.md`.
For benchmark results, see `benches/p5_RESULTS.md`.
