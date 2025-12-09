# Facade API - Simple Unified Interface

The Facade API provides a clean, customer-facing interface that hides implementation complexity and auto-selects the best deduplication pipeline for your use case.

## Quick Start

```rust
use kindly_dedup::Dedup;

// Auto-select best implementation
let mut dedup = Dedup::new(1_000_000)?;

// Add documents
for (id, text) in documents {
    dedup.add_document(id, text)?;
}

// Find duplicates
let clusters = dedup.find_duplicates(0.85)?;
println!("Found {} duplicate groups", clusters.len());
```

## Key Benefits

1. **Simple API**: Single `Dedup` struct, no configuration complexity
2. **Auto-Tuning**: Automatically selects best implementation based on corpus size and hardware
3. **Customer-Friendly**: No technical jargon ("capsule", "T0-T11", "lockfree", "Chaos")
4. **Unified Interface**: All implementations share the same API

## API Reference

### Creating a Deduplication Instance

#### Auto-Selection (Recommended)

```rust
// Automatically selects best mode based on corpus size
let dedup = Dedup::new(estimated_docs)?;
```

**Selection Logic:**
- **Default**: Streaming CPU processing (handles all corpus sizes efficiently)
- **GPU available** (requires `gpu-hybrid` feature): Uses GPU if expected speedup ≥2× and corpus ≥10K docs

#### Explicit Mode

```rust
use kindly_dedup::DedupMode;

// Force specific mode
let dedup = Dedup::with_mode(DedupMode::CpuStreaming, 500_000)?;
```

**Available Modes:**
- `DedupMode::Auto` - Auto-select (recommended)
- `DedupMode::CpuStreaming` - Streaming CPU processing
- `DedupMode::Gpu` - GPU-accelerated (requires `gpu-hybrid` feature)

### Adding Documents

```rust
// Add documents one at a time
dedup.add_document(0, "The quick brown fox")?;
dedup.add_document(1, "A lazy dog sleeps")?;

// Documents are batched internally for efficiency
```

**Notes:**
- Documents are buffered internally (batch size: 1000)
- Buffer is flushed automatically when `find_duplicates` is called
- Document IDs must be unique (u64)

### Finding Duplicates

```rust
// Find duplicates with similarity threshold
let clusters = dedup.find_duplicates(0.85)?;

// Process results
for cluster in clusters {
    println!("Duplicate group: {:?}", cluster);
}
```

**Threshold:**
- Range: 0.0 to 1.0
- Recommended: 0.80-0.90
- Higher = stricter (fewer false positives, more false negatives)
- Lower = looser (more false positives, fewer false negatives)

**Returns:**
- `Vec<Vec<u64>>` - Vector of clusters
- Each cluster contains document IDs that are duplicates
- Clusters with size 1 = unique documents (no duplicates)

### Statistics

```rust
let stats = dedup.stats();

println!("Documents processed: {}", stats.documents_processed);
println!("Total time: {:?}", stats.total_time);
println!("Avg time per doc: {:?}", stats.avg_time_per_doc);
println!("Mode: {:?}", stats.mode);
```

**Available Stats:**
- `documents_processed: usize` - Total documents added
- `duplicate_clusters: usize` - Number of clusters found (updated after `find_duplicates`)
- `total_time: Duration` - Total processing time
- `avg_time_per_doc: Duration` - Average time per document
- `mode: DedupMode` - Current execution mode
- `peak_memory_mb: Option<f64>` - Peak memory usage (if available)

## Examples

### Basic Workflow

```rust
use kindly_dedup::Dedup;

let mut dedup = Dedup::new(1000)?;

// Add documents
dedup.add_document(0, "The quick brown fox")?;
dedup.add_document(1, "A lazy dog sleeps")?;
dedup.add_document(2, "The quick brown fox")?; // Duplicate

// Find duplicates
let clusters = dedup.find_duplicates(0.85)?;

// Should find cluster: [0, 2]
for cluster in clusters {
    if cluster.len() > 1 {
        println!("Duplicates: {:?}", cluster);
    }
}
```

### Streaming Large Corpus

```rust
use kindly_dedup::Dedup;

// Auto-selects streaming mode for large corpus
let mut dedup = Dedup::new(10_000_000)?;

// Stream documents from file
for (id, line) in BufReader::new(File::open("corpus.txt")?).lines().enumerate() {
    dedup.add_document(id as u64, &line?)?;
}

// Find duplicates (buffered documents flushed automatically)
let clusters = dedup.find_duplicates(0.85)?;
println!("Found {} duplicate groups", clusters.len());
```

### Performance Monitoring

```rust
use kindly_dedup::Dedup;
use std::time::Instant;

let mut dedup = Dedup::new(100_000)?;

// Add documents
let start = Instant::now();
for (id, text) in documents {
    dedup.add_document(id, text)?;
}

// Get statistics
let stats = dedup.stats();
println!("Throughput: {:.0} docs/sec",
    stats.documents_processed as f64 / stats.total_time.as_secs_f64()
);

// Find duplicates
let clusters = dedup.find_duplicates(0.85)?;
println!("Found {} clusters in {:?}", clusters.len(), start.elapsed());
```

## Error Handling

```rust
use kindly_dedup::{Dedup, FacadeError};

match Dedup::new(1_000_000) {
    Ok(mut dedup) => {
        // Use dedup
        match dedup.find_duplicates(0.85) {
            Ok(clusters) => println!("Found {} clusters", clusters.len()),
            Err(FacadeError::Configuration(msg)) => eprintln!("Config error: {}", msg),
            Err(FacadeError::Pipeline(msg)) => eprintln!("Pipeline error: {}", msg),
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    Err(FacadeError::InvalidMode(msg)) => eprintln!("Invalid mode: {}", msg),
    Err(FacadeError::FeatureDisabled(msg)) => eprintln!("Feature disabled: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

**Error Types:**
- `FacadeError::Pipeline(String)` - Pipeline execution error
- `FacadeError::InvalidMode(String)` - Invalid mode selection (e.g., GPU not available)
- `FacadeError::Configuration(String)` - Configuration error (e.g., invalid threshold)
- `FacadeError::FeatureDisabled(String)` - Feature not enabled (e.g., GPU without `gpu-hybrid` feature)

## Feature Flags

The facade automatically adapts based on enabled features:

```toml
# Cargo.toml

[dependencies]
kindly_dedup = { version = "3.1", features = ["gpu-hybrid"] }
```

**Available Features:**
- `gpu-hybrid` - Enable GPU acceleration (requires GPU hardware)
- `benchmarking` - Enable benchmarking utilities

## Performance Tips

1. **Estimate Documents Accurately**: Better estimates enable better auto-selection
2. **Use Batching**: The facade batches documents internally for efficiency
3. **Choose Right Threshold**: 0.85 is a good default for most use cases
4. **Monitor Stats**: Use `stats()` to track throughput and adjust as needed

## Migration from Legacy APIs

### From `DedupPipeline`

```rust
// Old (deprecated)
let mut pipeline = DedupPipeline::new(num_documents);
pipeline.add_document(id, text);
let clusters = pipeline.find_duplicates(0.85);

// New (facade)
let mut dedup = Dedup::new(num_documents)?;
dedup.add_document(id, text)?;
let clusters = dedup.find_duplicates(0.85)?;
```

### From `UniversalDedupPipeline`

```rust
// Old (complex)
let pipeline = UniversalDedupPipeline::new(
    corpus_path,
    capacity,
    threshold,
    start_doc_id,
    end_doc_id,
)?;

// New (simple)
let mut dedup = Dedup::new(capacity)?;
// Add documents from corpus_path
let clusters = dedup.find_duplicates(threshold)?;
```

## See Also

- `/examples/facade_demo.rs` - Complete working example
- `/src/facade.rs` - Implementation details
- `/docs/MIGRATION_v3.md` - Migration guide from legacy APIs

## Support

For issues or questions:
- GitHub: [kindly_dedup issues](https://github.com/kindlydedup/kindly_dedup/issues)
- Documentation: `/docs/`
- Examples: `/examples/`
