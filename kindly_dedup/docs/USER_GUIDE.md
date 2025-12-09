# kindly_dedup User Guide

**Version**: v3.1.0
**Status**: Production Ready
**Target Audience**: Developers integrating LLM dataset deduplication

## Table of Contents

1. [Quick Start](#quick-start)
2. [Installation](#installation)
3. [CLI Usage](#cli-usage)
4. [Pipeline Selection](#pipeline-selection)
5. [Configuration](#configuration)
6. [Output Formats](#output-formats)
7. [Troubleshooting](#troubleshooting)

---

## Quick Start

### 10-Line Minimum Example

```rust
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;

fn main() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(1_000, &cpu_caps);

    pipeline.add_document(0, "The quick brown fox");
    pipeline.add_document(1, "The quick brown fox");  // duplicate

    let clusters = pipeline.find_duplicates(0.85);
    println!("Found {} duplicate clusters", clusters.len());
}
```

**Expected Output**: `Found 1 duplicate clusters`

---

## Installation

### Option 1: From Source (Recommended)

```bash
# Clone repository
git clone https://github.com/your-org/kindly_dedup.git
cd kindly_dedup

# Build release binary
cargo build --release

# Verify installation
./target/release/kindly_dedup --version
```

### Option 2: Add as Dependency

**Cargo.toml**:
```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", version = "3.1" }
```

### Build from Project

```bash
# Minimal (stable Rust)
cargo build --release

# SIMD acceleration (nightly Rust, 7.1× speedup)
rustup install nightly
cargo +nightly build --release --features simd-minhash

# Full features (GPU acceleration, SIMD, protection)
cargo +nightly build --release --all-features
```

---

## CLI Usage

### Main Binary Commands

kindly_dedup provides several binaries for different use cases:

#### 1. Interactive Client Demo

```bash
# 3-tier sales demo (100K/1M/10M docs)
cargo run --bin client_demo --release --features "benchmarking,persistent-dedup,meta-capsule"
```

**Use Cases**: Sales demonstrations, proof-of-concept, interactive exploration

#### 2. Audit Viewer (Q34 Compliance)

```bash
# Verify audit trail integrity
cargo run --bin audit_viewer --release --features benchmarking -- \
    verify target/criterion/audit_trail.jsonl

# View audit events
cargo run --bin audit_viewer --release --features benchmarking -- \
    view target/criterion/audit_trail.jsonl
```

**Use Cases**: SOX/SOC2/GDPR compliance verification, security audits

#### 3. Deduplication Server (HTTP API)

```bash
# Start HTTP server on port 8080
cargo run --bin dedup_server --release --features http-server -- \
    --port 8080 \
    --capacity 10000000
```

**Use Cases**: Production deployments, remote deduplication, API integration

### Command-Line Options

#### Global Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--threshold` | `-t` | Jaccard similarity threshold (0.0-1.0) | 0.85 |
| `--output` | `-o` | Output file path | stdout |
| `--format` | `-f` | Output format (json/csv/text) | json |
| `--verbose` | `-v` | Verbose logging | false |
| `--help` | `-h` | Show help message | - |
| `--version` | `-V` | Show version | - |

#### Pipeline-Specific Options

| Option | Description | Default |
|--------|-------------|---------|
| `--capacity` | Maximum documents | 1,000,000 |
| `--threads` | Worker threads (0=auto) | 0 |
| `--mode` | Pipeline mode (universal/hybrid/persistent/streaming) | universal |
| `--gpu` | Enable GPU acceleration | false |
| `--audit` | Enable Q34 audit trail | false |

### Input Formats

#### JSONL (JSON Lines)

```bash
# Process JSONL corpus
cargo run --release -- \
    --input corpus.jsonl \
    --format jsonl \
    --threshold 0.85
```

**Expected Format**:
```jsonl
{"id": 0, "text": "Document text here"}
{"id": 1, "text": "Another document"}
```

#### CSV

```bash
# Process CSV corpus
cargo run --release -- \
    --input corpus.csv \
    --format csv \
    --threshold 0.85
```

**Expected Format**:
```csv
id,text
0,"Document text here"
1,"Another document"
```

#### Raw Text (one document per line)

```bash
# Process raw text
cargo run --release -- \
    --input corpus.txt \
    --format text \
    --threshold 0.85
```

**Expected Format**:
```
Document text here
Another document
```

---

## Pipeline Selection

### Decision Matrix

| Pipeline | Memory @ 1M Docs | Memory @ 10M Docs | Throughput | Use Case |
|----------|------------------|-------------------|------------|----------|
| **UniversalDedupPipeline** | Auto | Auto | 60K docs/sec | **RECOMMENDED** - Auto-selects optimal mode |
| **DedupPipeline** | 256 MB | 2.56 GB | 60K docs/sec | Small-scale (<10M docs) |
| **HybridDedupPipeline** | 256 MB | 2.56 GB | 150K-1M docs/sec | GPU acceleration available |
| **PersistentDedupPipeline** | 3.5 GB | 3.5 GB | 373K docs/sec | Low-memory systems (93% reduction) |
| **StreamingDedupPipeline** | 273 MB | 273 MB | 100K docs/sec | Massive corpora (1-10B docs) |
| **ParallelDedupPipeline** | 256 MB | 2.56 GB | 6K docs/sec | **DEPRECATED** - Use UniversalDedupPipeline instead |

### Recommended Pipelines

#### 1. UniversalDedupPipeline (RECOMMENDED)

**Use When**: Default choice for all use cases

```rust
use kindly_dedup::UniversalDedupPipeline;

let mut pipeline = UniversalDedupPipeline::new(
    1_000_000,  // capacity
    0.85,       // threshold
    None        // auto-detect mode
)?;

// Add documents
for (id, text) in documents {
    pipeline.add_document(id, text)?;
}

// Find duplicates
let clusters = pipeline.find_duplicates()?;
```

**Features**:
- Auto-selects optimal mode (persistent/streaming/in-memory)
- Memory-aware tier selection
- GPU auto-fallback to CPU
- Zero configuration required

#### 2. HybridDedupPipeline (GPU Acceleration)

**Use When**: GPU available, need maximum throughput

```rust
use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
use atomic_capsule::CpuCapabilityCapsule;

let cpu_caps = CpuCapabilityCapsule::detect();
let mut pipeline = HybridDedupPipeline::new(
    1_000_000,
    PipelineMode::Auto,  // Auto-detect GPU/CPU
    &cpu_caps
)?;

// Add documents (auto-dispatches to GPU or CPU)
for (id, text) in documents {
    pipeline.add_document(id, text)?;
}

let clusters = pipeline.find_duplicates(0.85)?;
println!("Using GPU: {}", pipeline.is_using_gpu());
```

**Performance Targets**:
- iGPU (Ryzen): 150K docs/sec (2× vs CPU)
- GTX 1650: 300K docs/sec (4× vs CPU)
- RTX 3060: 500K docs/sec (7× vs CPU)
- RTX 4090: 1M docs/sec (14× vs CPU)

**Fallback**: Automatically falls back to CPU if GPU unavailable

#### 3. PersistentDedupPipeline (Low Memory)

**Use When**: Limited RAM (<4 GB available), need crash recovery

```rust
use kindly_dedup::PersistentDedupPipeline;

let mut pipeline = PersistentDedupPipeline::create(
    "dedup.mmap",  // memory-mapped file
    10_000_000     // capacity
)?;

// Add documents (persisted to disk)
for (id, text) in documents {
    pipeline.add_document(id, text)?;
}

// Check if document is duplicate
let is_dup = pipeline.is_duplicate("Document text")?;
```

**Benefits**:
- 93% memory reduction (3.5 GB vs 40 GB @ 10M docs)
- Crash-safe (atomic generation counters)
- Incremental updates (200× faster than rebuild)

#### 4. StreamingDedupPipeline (Massive Scale)

**Use When**: 1-10 billion documents, O(1) memory required

```rust
use kindly_dedup::streaming::StreamingDedupPipelineCapsule;

let mut pipeline = StreamingDedupPipelineCapsule::new(
    "corpus.jsonl",  // input path
    1_000_000_000,   // 1 billion docs
    0.85             // threshold
)?;

// Process entire corpus in single pass
pipeline.process_corpus("corpus.jsonl")?;

let clusters = pipeline.find_duplicates()?;
println!("Memory: {} MB (O(1))", pipeline.memory_usage_mb());
```

**Memory**: 273 MB constant (independent of corpus size)

---

## Configuration

### Environment Variables

#### Protection Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `KINDLY_PROTECTION_LEVEL` | Protection tier (0-4) | 0 |
| `KINDLY_LICENSE_KEY` | License key for commercial use | - |
| `KINDLY_HARDWARE_ID` | Hardware ID for binding | auto |
| `KINDLY_TRIAL_MODE` | Enable trial mode (true/false) | false |

**Example**:
```bash
export KINDLY_PROTECTION_LEVEL=2
export KINDLY_LICENSE_KEY="your-license-key"
cargo run --release --features meta-capsule
```

#### Audit Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `KINDLY_AUDIT_ENABLED` | Enable Q34 audit trail | false |
| `KINDLY_AUDIT_PATH` | Audit log file path | audit_trail.jsonl |
| `KINDLY_AUDIT_HASH_CHAIN` | Enable hash-chain integrity | true |

**Example**:
```bash
export KINDLY_AUDIT_ENABLED=true
export KINDLY_AUDIT_PATH=./logs/audit.jsonl
cargo run --release --features audit-trail
```

### Feature Flags

#### Production Features (Stable)

| Feature | Description | Performance |
|---------|-------------|-------------|
| `std` | Standard library support | Required |
| `parallel-dedup` | Multi-threaded processing | 8-12× speedup |
| `persistent-dedup` | Persistent mmap storage | 93% memory reduction |
| `bloom-prefilter` | Bloom pre-filtering | 2-10× on duplicates |
| `batch-lsh` | Batch LSH lookups | 1.5× speedup |
| `audit-trail` | Q34 compliance logging | <0.1% overhead |
| `meta-capsule` | Hardware-bound protection | <0.1% overhead |
| `gpu` | GPU acceleration core | 2-14× speedup |
| `gpu-hybrid` | GPU hybrid pipeline | 2-14× speedup |

#### Nightly Features (SIMD Acceleration)

| Feature | Description | Performance |
|---------|-------------|-------------|
| `simd-minhash` | SIMD MinHash (portable_simd) | 7.1× speedup |
| `simd-text-hashing` | SIMD FNV-1a text hashing | 4× speedup |
| `avx512-minhash` | AVX-512 MinHash (16-lane) | 2× vs AVX2 |
| `cache-optimized-minhash` | Cache-friendly layout | 1.3× speedup |

#### Full Feature Set

```toml
[dependencies]
kindly_dedup = { path = "../kindly_dedup", features = [
    # Core
    "std",
    "parallel-dedup",
    "persistent-dedup",

    # Optimization
    "bloom-prefilter",
    "batch-lsh",

    # SIMD (nightly)
    "simd-minhash",
    "simd-text-hashing",

    # GPU
    "gpu",
    "gpu-hybrid",

    # Compliance
    "audit-trail",
    "meta-capsule",
] }
```

### Memory/Performance Tuning

#### Auto-Tier Selection

```rust
use kindly_dedup::UniversalDedupPipeline;

// Auto-detect memory tier (sysinfo feature required)
let mut pipeline = UniversalDedupPipeline::new(
    10_000_000,
    0.85,
    None  // auto-detect
)?;
```

**Tier Selection Logic**:
- <2 GB RAM: StreamingDedupPipeline (273 MB)
- 2-8 GB RAM: PersistentDedupPipeline (3.5 GB)
- >8 GB RAM: DedupPipeline (in-memory)

#### Manual Tier Selection

```rust
use kindly_dedup::universal::{PipelineMode};

let mut pipeline = UniversalDedupPipeline::new(
    10_000_000,
    0.85,
    Some(PipelineMode::Persistent)  // Force persistent mode
)?;
```

#### Thread Count Tuning

```rust
// Auto-detect threads (recommended)
let num_threads = std::thread::available_parallelism()?.get();

// Manual override (for testing)
let num_threads = 8;

let mut pipeline = ParallelDedupPipeline::new(
    1_000_000,
    num_threads
)?;
```

---

## Output Formats

### JSON Clusters

**Format**: Array of document ID clusters

```bash
cargo run --release -- \
    --input corpus.jsonl \
    --output duplicates.json \
    --format json
```

**Output** (`duplicates.json`):
```json
[
  [0, 1, 2],
  [5, 8],
  [12, 15, 19]
]
```

**Interpretation**:
- Cluster 1: Documents 0, 1, 2 are duplicates (≥85% similar)
- Cluster 2: Documents 5, 8 are duplicates
- Cluster 3: Documents 12, 15, 19 are duplicates

### CSV Output

**Format**: CSV with cluster ID and document ID

```bash
cargo run --release -- \
    --input corpus.jsonl \
    --output duplicates.csv \
    --format csv
```

**Output** (`duplicates.csv`):
```csv
cluster_id,document_id
0,0
0,1
0,2
1,5
1,8
2,12
2,15
2,19
```

### Statistics/Metrics

**Built-in Metrics** (logged automatically):

```rust
let clusters = pipeline.find_duplicates(0.85)?;

// Metrics available
let total_docs = 1_000_000;
let num_clusters = clusters.len();
let duplicate_docs = clusters.iter().map(|c| c.len()).sum::<usize>();
let unique_docs = total_docs - duplicate_docs;
let duplicate_rate = duplicate_docs as f64 / total_docs as f64;

println!("Total documents: {}", total_docs);
println!("Unique documents: {}", unique_docs);
println!("Duplicate clusters: {}", num_clusters);
println!("Duplicate rate: {:.2}%", duplicate_rate * 100.0);
```

**Expected Output**:
```
Total documents: 1000000
Unique documents: 800000
Duplicate clusters: 50000
Duplicate rate: 20.00%
```

---

## Troubleshooting

### Issue: Out of Memory

**Symptoms**: Process killed with "Out of memory" error

**Solutions**:

1. **Use Persistent Mode** (93% memory reduction):
   ```toml
   features = ["persistent-dedup"]
   ```

2. **Use Streaming Mode** (O(1) memory):
   ```rust
   use kindly_dedup::streaming::StreamingDedupPipelineCapsule;
   ```

3. **Process in Batches**:
   ```rust
   let batch_size = 10_000;
   for chunk in corpus.chunks(batch_size) {
       pipeline.add_documents(chunk)?;
   }
   ```

4. **Check Available Memory**:
   ```bash
   free -h  # Linux
   vm_stat  # macOS
   ```

### Issue: Slow Performance (<100K docs/sec)

**Symptoms**: Throughput below expected baseline

**Solutions**:

1. **Enable SIMD** (7.1× speedup):
   ```bash
   rustup install nightly
   cargo +nightly build --release --features simd-minhash
   ```

2. **Enable GPU Acceleration** (2-14× speedup):
   ```toml
   features = ["gpu", "gpu-hybrid"]
   ```

3. **Profile Bottlenecks**:
   ```bash
   cargo flamegraph --release --features benchmarking
   open flamegraph.svg
   ```

4. **Check Disk I/O** (use SSD, not HDD):
   ```bash
   iostat -x 1  # Linux
   ```

### Issue: Low Accuracy (<85% F1)

**Symptoms**: Missing many duplicate pairs

**Solutions**:

1. **Lower Threshold** (catch more duplicates):
   ```rust
   let clusters = pipeline.find_duplicates(0.75)?;  // Was 0.85
   ```

2. **Check Tokenization** (need 10+ tokens):
   ```rust
   use atomic_capsule::probabilistic::tokenize;
   let tokens = tokenize("your document text");
   println!("Tokens: {:?} ({})", tokens, tokens.len());
   ```

3. **Verify LSH Parameters** (Phase 11 adaptive):
   ```rust
   // Automatic in Phase 11+ (12×10 @ 10M docs)
   // Manual override (advanced):
   pipeline.set_lsh_params(bands, rows);
   ```

### Issue: GPU Fallback to CPU

**Symptoms**: `is_using_gpu()` returns `false`

**Solutions**:

1. **Check GPU Availability**:
   ```bash
   # NVIDIA
   nvidia-smi

   # AMD
   rocm-smi

   # Intel
   clinfo
   ```

2. **Verify wgpu Backend**:
   ```rust
   use kindly_dedup::gpu::capabilities::detect_gpu;

   if let Some(gpu_info) = detect_gpu() {
       println!("GPU: {} ({})", gpu_info.name, gpu_info.backend);
   } else {
       println!("No GPU detected");
   }
   ```

3. **Force CPU Mode** (if GPU broken):
   ```rust
   let mut pipeline = HybridDedupPipeline::new(
       1_000_000,
       PipelineMode::Cpu,  // Force CPU
       &cpu_caps
   )?;
   ```

### Issue: Compilation Errors

**Symptoms**: Missing feature errors, unresolved imports

**Solutions**:

1. **Check Feature Flags**:
   ```toml
   # Minimal (stable)
   features = ["std"]

   # Recommended
   features = ["std", "parallel-dedup", "persistent-dedup"]
   ```

2. **Use Nightly for SIMD**:
   ```bash
   rustup install nightly
   cargo +nightly build --features simd-minhash
   ```

3. **Clean Build**:
   ```bash
   cargo clean
   cargo build --release
   ```

4. **Update Dependencies**:
   ```bash
   cargo update
   cargo build --release
   ```

### Issue: Audit Trail Verification Failed

**Symptoms**: Hash chain integrity check fails

**Solutions**:

1. **Verify Audit Log Exists**:
   ```bash
   ls -lh target/criterion/audit_trail.jsonl
   ```

2. **Run Audit Viewer**:
   ```bash
   cargo run --bin audit_viewer --release --features benchmarking -- \
       verify target/criterion/audit_trail.jsonl
   ```

3. **Check Disk Corruption**:
   ```bash
   # Verify file integrity
   sha256sum target/criterion/audit_trail.jsonl
   ```

4. **Regenerate Audit Trail**:
   ```bash
   rm target/criterion/audit_trail.jsonl
   cargo bench --features benchmarking
   ```

---

## Advanced Topics

### Custom Threshold Selection

**Trade-off**: Higher threshold = fewer false positives, lower recall

| Threshold | Precision | Recall | Use Case |
|-----------|-----------|--------|----------|
| 0.90 | 98% | 75% | High precision (few false positives) |
| 0.85 | 94% | 85% | **Balanced (default)** |
| 0.75 | 85% | 95% | High recall (catch more duplicates) |

### Batch Processing

```rust
use kindly_dedup::DedupPipeline;

let mut pipeline = DedupPipeline::new(10_000_000, &cpu_caps);

// Process 100K documents per batch
let batch_size = 100_000;
for (batch_idx, chunk) in corpus.chunks(batch_size).enumerate() {
    let doc_refs: Vec<_> = chunk.iter()
        .map(|(id, text)| (*id, text.as_str()))
        .collect();

    pipeline.add_documents(&doc_refs)?;

    println!("Batch {}: {} docs processed", batch_idx + 1, chunk.len());
}

let clusters = pipeline.find_duplicates(0.85)?;
```

### Incremental Updates

```rust
use kindly_dedup::PersistentDedupPipeline;

// Initial build (10M docs, ~75 seconds)
let mut pipeline = PersistentDedupPipeline::create("dedup.mmap", 10_000_000)?;
for (id, text) in initial_corpus {
    pipeline.add_document(id, text)?;
}

// Weekly update (100K new docs, ~30 seconds, 200× faster)
let new_docs = load_new_documents()?;
pipeline.rebuild_incremental(&new_docs)?;
```

### Error Handling

```rust
use kindly_dedup::{ParallelDedupPipeline, pipeline::PipelineError};

fn process_corpus() -> Result<(), PipelineError> {
    let mut pipeline = ParallelDedupPipeline::new(1_000_000, 16)
        .map_err(|e| PipelineError::from(e))?;

    for (id, text) in corpus {
        pipeline.add_document(id, text)?;
    }

    let clusters = pipeline.find_duplicates(0.85)?;

    Ok(())
}
```

---

## Support & References

### Documentation

- **CLAUDE.md**: `/home/samuel/Primitives/kindly_dedup/CLAUDE.md` - Architecture details
- **GETTING_STARTED.md**: `/home/samuel/Primitives/kindly_dedup/docs/GETTING_STARTED.md` - Tutorials
- **STREAMING_USER_GUIDE.md**: `/home/samuel/Primitives/kindly_dedup/docs/STREAMING_USER_GUIDE.md` - Streaming pipeline
- **FEATURES.md**: `/home/samuel/Primitives/kindly_dedup/docs/FEATURE_FLAGS.md` - Feature flag reference

### Examples

- **Basic Pipeline**: `examples/load_jsonl.rs`
- **GPU Acceleration**: `examples/adaptive_selection.rs`
- **Streaming**: `examples/streaming_dedup_pipeline.rs`
- **Batch Processing**: `examples/load_with_progress.rs`

### Framework Compliance

- **UCE34**: Q1-Q34 systematic discovery
- **Chaos**: 100% lockfree computational capsules
- **ASSUM**: 99.99% safety (all assumptions verified)
- **B32**: Fair benchmarking (95% CI, 1000+ iterations)
- **T28**: Comprehensive testing (7,642+ tests)
- **I20**: Integration validation (20/20 questions)

---

**Guide Version**: v3.1.0
**Last Updated**: 2025-11-26
**Status**: Production Ready
**Framework**: UCE34 + Chaos + ASSUM + B32 + T28 + I20
