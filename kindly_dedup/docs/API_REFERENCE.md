# API Reference - kindly_dedup v3.1.0

**Production-ready LLM training dataset deduplication using computational capsules**

This document provides complete API reference for the kindly_dedup library v3.1.0.

## Table of Contents

1. [Core Pipeline APIs](#core-pipeline-apis)
2. [Configuration Types](#configuration-types)
3. [Result Types](#result-types)
4. [Error Handling](#error-handling)
5. [Advanced Features](#advanced-features)
6. [Code Examples](#code-examples)

---

## Core Pipeline APIs

### UniversalDedupPipeline (RECOMMENDED)

**Tier**: T6 Mixed (orchestrates T9+T10+T5+T1)
**Memory**: O(1) constant - 1.44 GB (independent of corpus size)
**Throughput**: 100K+ docs/sec
**Status**: ✅ Production-ready

```rust
use kindly_dedup::universal::UniversalDedupPipeline;

// Create new pipeline
pub fn new(
    corpus_path: impl AsRef<Path>,
    num_documents: usize,
    threshold: f64
) -> Result<Self, UniversalPipelineError>

// Process entire corpus
pub fn process_corpus(&mut self) -> Result<(), UniversalPipelineError>

// Find duplicate clusters
pub fn find_duplicates(&self) -> Result<Vec<Vec<u64>>, UniversalPipelineError>

// Get current progress (for TUI/monitoring)
pub fn progress(&self) -> PipelineProgress

// Check current phase
pub fn current_phase(&self) -> Phase
```

**Example**:
```rust
use kindly_dedup::universal::UniversalDedupPipeline;

let mut pipeline = UniversalDedupPipeline::new(
    "corpus.jsonl",
    1_000_000_000,  // 1B documents
    0.85            // 85% Jaccard threshold
)?;

pipeline.process_corpus()?;
let clusters = pipeline.find_duplicates()?;
println!("Found {} duplicate clusters", clusters.len());
```

---

### DedupPipeline (Legacy, Deprecated)

**Status**: ⚠️ Deprecated (use UniversalDedupPipeline instead)
**Memory**: O(n) - scales with document count
**Throughput**: 60K docs/sec @ 1 thread

```rust
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;

// Create new pipeline
pub fn new(
    num_documents: usize,
    cpu_caps: &CpuCapabilityCapsule
) -> Self

// Add single document
pub fn add_document(
    &mut self,
    doc_id: DocId,
    text: &str
) -> Result<(), PipelineError>

// Find duplicates with threshold
pub fn find_duplicates(
    &self,
    threshold: JaccardThreshold
) -> Result<Vec<Cluster>, PipelineError>

// Get statistics
pub fn documents_added(&self) -> usize
pub fn documents_skipped(&self) -> usize
```

**Example**:
```rust
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;

let cpu_caps = CpuCapabilityCapsule::detect();
let mut pipeline = DedupPipeline::new(10000, &cpu_caps);

pipeline.add_document(0, "The quick brown fox")?;
pipeline.add_document(1, "The quick brown fox jumps")?;
pipeline.add_document(2, "A completely different text")?;

let clusters = pipeline.find_duplicates(0.85)?;
println!("Found {} clusters", clusters.len());
```

---

### PersistentDedupPipeline (Streaming + Persistent)

**Tier**: T9 (Persistent) + T10 (Probabilistic)
**Memory**: O(1) - 3.5 GB constant
**Status**: ✅ Production-ready
**Use Case**: Incremental updates, crash recovery

```rust
use kindly_dedup::PersistentDedupPipeline;

// Create new persistent pipeline
pub fn create(
    path: impl AsRef<Path>,
    capacity: usize
) -> Result<Self, PersistentError>

// Load existing pipeline
pub fn load(
    path: impl AsRef<Path>
) -> Result<Self, PersistentError>

// Add document
pub fn add_document(
    &mut self,
    doc_id: u64,
    text: &str
) -> Result<(), PersistentError>

// Incremental rebuild with new documents
pub fn rebuild_incremental(
    &mut self,
    new_docs: &[(u64, &str)]
) -> Result<(), PersistentError>

// Check if text is duplicate
pub fn is_duplicate(&self, text: &str) -> Result<bool, PersistentError>
```

**Example**:
```rust
use kindly_dedup::PersistentDedupPipeline;

// Create persistent pipeline
let mut pipeline = PersistentDedupPipeline::create(
    "dedup.mmap",
    10_000_000
)?;

pipeline.add_document(0, "First document")?;

// Later: incremental update
let new_docs = vec![(1000, "New document")];
pipeline.rebuild_incremental(&new_docs)?;

// Check for duplicates
if pipeline.is_duplicate("First document")? {
    println!("Duplicate found!");
}
```

---

### HybridDedupPipeline (GPU Acceleration)

**Tier**: T7 Heterogeneous (CPU+GPU coordination)
**Status**: ✅ Production-ready (62 tests passing)
**Requires**: `gpu-hybrid` feature flag
**Speedup**: 2-14× (iGPU 2×, RTX 4090 14×)

```rust
use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
use atomic_capsule::CpuCapabilityCapsule;

// Create hybrid pipeline with auto GPU detection
pub fn new(
    num_documents: usize,
    mode: PipelineMode,
    cpu_caps: &CpuCapabilityCapsule
) -> Result<Self, Error>

// Add document (auto-dispatches to GPU or CPU)
pub fn add_document(
    &mut self,
    id: u64,
    text: &str
) -> Result<(), Error>

// Find duplicates
pub fn find_duplicates(
    &self,
    threshold: f64
) -> Result<Vec<Vec<u64>>, Error>

// Check if GPU is being used
pub fn is_using_gpu(&self) -> bool

// Get statistics
pub fn stats(&self) -> HybridPipelineStats
```

**Example**:
```rust
use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
use atomic_capsule::CpuCapabilityCapsule;

let cpu_caps = CpuCapabilityCapsule::detect();
let mut pipeline = HybridDedupPipeline::new(
    10_000,
    PipelineMode::Auto,  // Auto GPU/CPU selection
    &cpu_caps
)?;

for (id, text) in documents {
    pipeline.add_document(id, text)?;
}

let clusters = pipeline.find_duplicates(0.85)?;
println!("Using GPU: {}", pipeline.is_using_gpu());
```

---

## Configuration Types

### DedupConfig

```rust
pub struct DedupConfig {
    /// Number of documents expected
    pub num_documents: usize,

    /// Jaccard similarity threshold (0.0-1.0)
    pub threshold: f64,

    /// Number of hash functions for MinHash (default: 128)
    pub num_hashes: usize,

    /// LSH parameter: number of tables (default: 5)
    pub lsh_tables: usize,

    /// Enable Bloom pre-filter (default: true)
    pub use_bloom_filter: bool,

    /// Enable exact deduplication first pass (default: true)
    pub use_exact_dedup: bool,
}

impl Default for DedupConfig {
    fn default() -> Self {
        Self {
            num_documents: 10_000,
            threshold: 0.85,
            num_hashes: 128,
            lsh_tables: 5,
            use_bloom_filter: true,
            use_exact_dedup: true,
        }
    }
}
```

### PipelineMode (GPU)

```rust
pub enum PipelineMode {
    /// Automatic GPU/CPU selection based on hardware
    Auto,

    /// Force CPU execution
    Cpu,

    /// Force GPU execution (fails if GPU unavailable)
    Gpu,
}
```

### Phase (Progress Tracking)

```rust
#[repr(u64)]
pub enum Phase {
    Read = 0,      // Reading corpus
    Sign = 1,      // Computing signatures
    Hash = 2,      // Building LSH buckets
    Cluster = 3,   // Clustering duplicates
    Output = 4,    // Writing results
}
```

---

## Result Types

### Cluster

```rust
// Duplicate cluster (vec of document IDs)
pub type Cluster = Vec<DocId>;

// Example: [[0, 1, 5], [2, 3], [4]] means:
// - Docs 0,1,5 are duplicates
// - Docs 2,3 are duplicates
// - Doc 4 is unique
```

### DedupResult

```rust
pub struct DedupResult {
    /// Duplicate clusters
    pub clusters: Vec<Cluster>,

    /// Total documents processed
    pub docs_processed: usize,

    /// Documents identified as duplicates
    pub docs_duplicated: usize,

    /// Documents skipped by Bloom filter
    pub docs_skipped_bloom: usize,

    /// Documents skipped by exact dedup
    pub docs_skipped_exact: usize,

    /// Processing time (seconds)
    pub processing_time: f64,
}
```

### DedupStats

```rust
pub struct DedupStats {
    /// Documents added
    pub documents_added: usize,

    /// Documents skipped
    pub documents_skipped: usize,

    /// Exact duplicates found
    pub exact_duplicates: u64,

    /// LSH buckets created
    pub lsh_buckets: usize,

    /// MinHash comparisons performed
    pub minhash_comparisons: u64,
}
```

### PipelineProgress

```rust
pub struct PipelineProgress {
    /// Current phase (0-4)
    pub current_phase: u64,

    /// Documents processed
    pub docs_processed: u64,

    /// Total documents
    pub docs_total: u64,

    /// Error count
    pub error_count: u64,
}
```

---

## Error Handling

### PipelineError

```rust
#[derive(Debug)]
pub enum PipelineError {
    /// Document ID out of bounds
    DocumentIdOutOfBounds {
        doc_id: usize,
        capacity: usize,
    },

    /// Protection violation (when binary-protection enabled)
    ProtectionViolation(ProtectionError),

    /// Signature not found
    SignatureNotFound { doc_id: usize },

    /// LSH bucketing error
    LshBucketingError { reason: String },

    /// Resource limit exceeded
    ResourceLimitExceeded { reason: String },

    /// Memory budget exceeded
    MemoryBudgetExceeded,

    /// Audit trail error (when audit-trail enabled)
    AuditError { reason: String },
}
```

### UniversalPipelineError

```rust
#[derive(Debug, Error)]
pub enum UniversalPipelineError {
    /// Phase transition failed
    #[error("Phase transition failed: expected {expected:?}, got {actual:?}")]
    PhaseTransitionFailed { expected: u64, actual: u64 },

    /// Capsule error
    #[error("Capsule error: {0}")]
    CapsuleError(String),

    /// Generation mismatch (crash recovery)
    #[error("Generation mismatch: {0}")]
    GenerationMismatch(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    /// License error
    #[error("License error: {0}")]
    LicenseError(#[from] LicenseError),

    /// Tamper detection
    #[error("Protection error: {0}")]
    TamperError(#[from] ProtectionError),
}
```

### LicenseError

```rust
#[derive(Debug, Error)]
pub enum LicenseError {
    /// License expired
    #[error("License expired at {0}")]
    Expired(u64),

    /// License revoked
    #[error("License revoked")]
    Revoked,

    /// GB limit exceeded
    #[error("GB limit exceeded: {used} / {limit}")]
    GbLimitExceeded { used: u64, limit: u64 },

    /// Invalid license key
    #[error("Invalid license key")]
    InvalidKey,

    /// Tamper detected
    #[error("License tampered: checksum mismatch")]
    TamperDetected,
}
```

---

## Advanced Features

### License Validation

```rust
use kindly_dedup::license_capsule::{LicenseCapsule, LicenseTier, LicenseStatus};

// Create license
let license = LicenseCapsule::new("LICENSE-KEY-XXXXX", LicenseTier::Pro)?;

// Validate before processing
match license.validate()? {
    LicenseStatus::Valid => {
        // Record usage (10 GB)
        license.record_usage(10)?;
    },
    LicenseStatus::Expired => {
        return Err("License expired".into());
    },
    LicenseStatus::Revoked => {
        return Err("License revoked".into());
    },
}

// Check remaining quota
if let Some(remaining) = license.remaining_gb() {
    println!("GB remaining: {}", remaining);
}
```

### Protection System

```rust
use kindly_dedup::protection::{check_protection, init_protection};

// Initialize protection (4-layer tamper detection)
init_protection()?;

// Check protection status
match check_protection() {
    Ok(_) => println!("Protection verified"),
    Err(e) => eprintln!("Tamper detected: {}", e),
}
```

### Adaptive GPU/CPU Mode Selection

```rust
use kindly_dedup::adaptive::{
    AdaptivePipelineCapsule,
    AdaptivePipelineConfig,
    ExecutionMode,
};

// Create adaptive pipeline
let config = AdaptivePipelineConfig::default();
let mut pipeline = AdaptivePipelineCapsule::new(config);

// Record batch timing (CPU time, GPU time, docs)
pipeline.record_batch(1000, 800, 50_000);

// Check recommended mode
match pipeline.current_mode() {
    ExecutionMode::Cpu => println!("Using CPU"),
    ExecutionMode::Gpu => println!("Using GPU"),
}

// Should we switch to GPU?
if pipeline.should_use_gpu() {
    // Trigger GPU mode
}
```

### Format Detection

```rust
use kindly_dedup::format::{load_documents_auto, FormatReaderCapsule};

// Auto-detect format and load
let docs = load_documents_auto("corpus.jsonl")?;
println!("Loaded {} documents", docs.len());

// Load with explicit format
let reader = FormatReaderCapsule::new("corpus.csv")?;
let docs = reader.read_all()?;
```

### Parallel Loading

```rust
use kindly_dedup::format::load_documents_parallel;

// Load multiple files in parallel (T4 Batch tier)
let files = vec!["file1.jsonl", "file2.jsonl", "file3.jsonl"];
let docs = load_documents_parallel(&files, 4)?;  // 4 threads
println!("Loaded {} documents", docs.len());
```

---

## Code Examples

### Basic Deduplication (5 lines)

```rust
use kindly_dedup::universal::UniversalDedupPipeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = UniversalDedupPipeline::new("corpus.jsonl", 1_000_000, 0.85)?;
    pipeline.process_corpus()?;
    let clusters = pipeline.find_duplicates()?;
    println!("Found {} duplicate clusters", clusters.len());
    Ok(())
}
```

### Streaming Large Corpus

```rust
use kindly_dedup::universal::UniversalDedupPipeline;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    // O(1) 1.44 GB memory, supports 1B+ documents
    let mut pipeline = UniversalDedupPipeline::new(
        "large_corpus.jsonl",
        1_000_000_000,  // 1B documents
        0.85
    )?;

    // Stream processing (O(1) memory)
    pipeline.process_corpus()?;

    // Find duplicates
    let clusters = pipeline.find_duplicates()?;

    let elapsed = start.elapsed();
    println!("Processed 1B documents in {:.2}s", elapsed.as_secs_f64());
    println!("Found {} clusters", clusters.len());
    println!("Throughput: {:.0} docs/sec",
             1_000_000_000.0 / elapsed.as_secs_f64());

    Ok(())
}
```

### GPU Acceleration

```rust
use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
use atomic_capsule::CpuCapabilityCapsule;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Auto GPU/CPU selection
    let mut pipeline = HybridDedupPipeline::new(
        1_000_000,
        PipelineMode::Auto,
        &cpu_caps
    )?;

    // Load documents
    for (id, text) in load_corpus()? {
        pipeline.add_document(id, text)?;
    }

    // Find duplicates
    let clusters = pipeline.find_duplicates(0.85)?;

    // Print stats
    let stats = pipeline.stats();
    println!("GPU used: {}", pipeline.is_using_gpu());
    println!("Throughput: {} docs/sec", stats.throughput);

    Ok(())
}
```

### Error Handling

```rust
use kindly_dedup::universal::{UniversalDedupPipeline, UniversalPipelineError};

fn process_corpus() -> Result<Vec<Vec<u64>>, UniversalPipelineError> {
    let mut pipeline = UniversalDedupPipeline::new(
        "corpus.jsonl",
        1_000_000,
        0.85
    )?;

    match pipeline.process_corpus() {
        Ok(_) => {
            println!("Processing complete");
        },
        Err(UniversalPipelineError::PhaseTransitionFailed { expected, actual }) => {
            eprintln!("Phase error: expected {}, got {}", expected, actual);
            return Err(UniversalPipelineError::PhaseTransitionFailed { expected, actual });
        },
        Err(UniversalPipelineError::IoError(e)) => {
            eprintln!("I/O error: {}", e);
            return Err(UniversalPipelineError::IoError(e));
        },
        Err(e) => {
            eprintln!("Pipeline error: {}", e);
            return Err(e);
        },
    }

    pipeline.find_duplicates()
}
```

### Incremental Updates

```rust
use kindly_dedup::PersistentDedupPipeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initial build
    let mut pipeline = PersistentDedupPipeline::create("dedup.mmap", 10_000_000)?;

    for (id, text) in initial_corpus {
        pipeline.add_document(id, text)?;
    }

    println!("Initial build complete");

    // Weekly update: add new documents
    let new_docs = load_new_documents()?;
    pipeline.rebuild_incremental(&new_docs)?;

    println!("Incremental update: {} new docs", new_docs.len());

    // Check for duplicates
    for text in check_texts {
        if pipeline.is_duplicate(text)? {
            println!("Duplicate: {}", text);
        }
    }

    Ok(())
}
```

### Progress Monitoring

```rust
use kindly_dedup::universal::{UniversalDedupPipeline, Phase};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = UniversalDedupPipeline::new("corpus.jsonl", 1_000_000, 0.85)?;

    // Spawn progress monitor thread
    let progress_handle = thread::spawn(move || {
        loop {
            let progress = pipeline.progress();

            match progress.current_phase {
                0 => println!("Reading: {}/{}", progress.docs_processed, progress.docs_total),
                1 => println!("Signing: {}/{}", progress.docs_processed, progress.docs_total),
                2 => println!("Hashing: {}/{}", progress.docs_processed, progress.docs_total),
                3 => println!("Clustering: {}/{}", progress.docs_processed, progress.docs_total),
                4 => println!("Writing: {}/{}", progress.docs_processed, progress.docs_total),
                _ => break,
            }

            thread::sleep(Duration::from_secs(1));
        }
    });

    // Process corpus
    pipeline.process_corpus()?;

    progress_handle.join().unwrap();

    Ok(())
}
```

---

## Performance Benchmarks

| Pipeline | Memory | Throughput | Speedup | Status |
|----------|--------|------------|---------|--------|
| UniversalDedupPipeline | O(1) 1.44 GB | 100K+ docs/sec | Baseline | ✅ Recommended |
| DedupPipeline | O(n) | 60K docs/sec | 38× vs Python | ⚠️ Deprecated |
| PersistentDedupPipeline | O(1) 3.5 GB | 373K docs/sec @ 16 cores | 200× incremental | ✅ Production |
| HybridDedupPipeline (GPU) | O(n) | 150K-1M docs/sec | 2-14× | ✅ Production |

---

## Feature Flags

```toml
[dependencies]
kindly_dedup = { version = "3.1.0", features = ["gpu-hybrid", "persistent-dedup"] }
```

| Feature | Description | Status |
|---------|-------------|--------|
| `default` | Standard deduplication | ✅ Stable |
| `persistent-dedup` | Persistent mmap-backed pipeline | ✅ Stable |
| `gpu-hybrid` | GPU acceleration (wgpu) | ✅ Stable |
| `adaptive-pipeline` | GPU/CPU mode switching | ✅ Stable |
| `parallel-dedup` | Multi-threaded processing | ⚠️ Experimental |
| `simd-minhash` | SIMD MinHash (nightly) | ✅ Stable |
| `audit-trail` | Q34 compliance logging | ✅ Stable |
| `interactive` | TUI progress dashboard | ✅ Stable |

---

## License Tiers

| Tier | Duration | Data Limit | Price | Features |
|------|----------|------------|-------|----------|
| Trial | 7 days | 100 GB | Free | Basic dedup, no commercial use |
| Starter | 1 year | 500 GB | $500 | Commercial use, email support |
| Pro | 1 year | Unlimited | $1500 | Priority support, SLA |
| Enterprise | Custom | Custom | $5000+ | Dedicated support, training |

---

## See Also

- [User Guide](USER_GUIDE.md) - Getting started, tutorials
- [Performance Guide](PERFORMANCE_GUIDE.md) - Optimization tips
- [Deployment Guide](DEPLOYMENT_GUIDE.md) - Production deployment
- [CLAUDE.md](../CLAUDE.md) - Complete project documentation
- [GitHub](https://github.com/your-org/kindly_dedup) - Source code

---

**Version**: 3.1.0
**Updated**: 2025-11-25
**Status**: Production-ready (T6 Mixed + T7 Heterogeneous + T9 Persistent + T10 Probabilistic)
