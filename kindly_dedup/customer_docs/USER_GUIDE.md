# User Guide

Comprehensive reference for Kindly Dedup command-line interface and features.

## Command-Line Interface

### Basic Syntax

```bash
kindly-dedup <command> [options]
```

### Commands

#### `deduplicate` - Main deduplication command

Process a dataset and identify duplicate documents.

```bash
kindly-dedup deduplicate [options]
```

**Required Options**:
- `--input <path>` - Input file path (JSONL, CSV, or TXT)
- `--output <path>` - Output file path (JSON format)

**Optional Parameters**:
- `--threshold <float>` - Similarity threshold (0.0-1.0, default: 0.85)
- `--threads <num>` - Number of processing threads (default: auto-detect)
- `--batch-size <num>` - Documents per batch (default: 1000)
- `--format <type>` - Input format: jsonl, csv, txt (default: auto-detect)
- `--persistent` - Enable persistent mode for large datasets
- `--storage-path <path>` - Storage directory for persistent mode
- `--incremental` - Enable incremental processing
- `--gpu` - Enable GPU acceleration (requires compatible hardware)

#### `verify` - Verify deduplication results

Check the integrity and accuracy of deduplication output.

```bash
kindly-dedup verify --input <original> --output <results>
```

#### `stats` - Show dataset statistics

Display information about a dataset without processing.

```bash
kindly-dedup stats --input <path>
```

#### `serve` - Start HTTP API server

Launch the REST API server for programmatic access.

```bash
kindly-dedup serve [options]
```

**Server Options**:
- `--host <address>` - Bind address (default: 127.0.0.1)
- `--port <num>` - Port number (default: 8080)
- `--workers <num>` - API worker threads (default: auto-detect)

## Input Formats

### JSONL (JSON Lines)

One JSON object per line with `id` and `text` fields:

```jsonl
{"id": 1, "text": "Document content here"}
{"id": 2, "text": "Another document"}
```

**Optional Fields**:
- `metadata`: Custom metadata (preserved in output)
- `timestamp`: Document timestamp (ISO 8601)
- `source`: Source identifier

### CSV

Comma-separated values with header row:

```csv
id,text
1,"First document"
2,"Second document"
```

**Supported Delimiters**: comma (`,`), tab (`\t`), pipe (`|`)

### Plain Text

One document per line (auto-generated sequential IDs):

```
This is the first document.
This is the second document.
```

## Output Format

### Standard Output (JSON)

```json
{
  "clusters": [
    {
      "representative_id": 1,
      "duplicate_ids": [3, 5, 7],
      "similarity": 0.95,
      "size": 4
    }
  ],
  "stats": {
    "total_documents": 10000,
    "unique_documents": 8500,
    "duplicate_documents": 1500,
    "deduplication_ratio": 0.15,
    "processing_time_ms": 1234,
    "throughput_docs_per_sec": 8097
  },
  "metadata": {
    "threshold": 0.85,
    "timestamp": "2025-11-25T10:30:00Z",
    "version": "3.0.0"
  }
}
```

### Cluster Details

Each cluster represents a group of similar documents:
- `representative_id`: The document chosen as the canonical version
- `duplicate_ids`: Array of document IDs that are duplicates
- `similarity`: Average similarity score within cluster (0.0-1.0)
- `size`: Total documents in cluster (including representative)

## Configuration

### Environment Variables

- `KINDLY_DEDUP_LICENSE` - License key (required for production use)
- `KINDLY_DEDUP_THREADS` - Default thread count
- `KINDLY_DEDUP_LOG_LEVEL` - Logging level: debug, info, warn, error
- `KINDLY_DEDUP_STORAGE` - Default storage path for persistent mode

### Configuration File

Create `dedup.toml` in working directory:

```toml
[processing]
threshold = 0.85
threads = 8
batch_size = 1000

[storage]
persistent = true
path = "./storage"
incremental = true

[output]
format = "json"
pretty = true
include_stats = true
```

Use with `--config dedup.toml` flag.

## Advanced Features

### Persistent Mode (Large Datasets)

For datasets larger than available RAM, use persistent mode:

```bash
kindly-dedup deduplicate \
  --input large_corpus.jsonl \
  --output results.json \
  --threshold 0.85 \
  --persistent \
  --storage-path ./dedup_storage
```

**Benefits**:
- Process billions of documents
- 93% memory reduction vs in-memory mode
- Crash recovery and resumable processing
- Incremental updates for new documents

**Storage Requirements**: ~5 KB per document

### Incremental Processing

Add new documents to existing deduplication state:

```bash
# Initial processing
kindly-dedup deduplicate \
  --input corpus_v1.jsonl \
  --output results_v1.json \
  --persistent \
  --storage-path ./storage

# Add new documents
kindly-dedup deduplicate \
  --input new_docs.jsonl \
  --output results_v2.json \
  --persistent \
  --storage-path ./storage \
  --incremental
```

**Performance**: 200× faster than reprocessing entire dataset

### GPU Acceleration

Enable GPU processing for supported hardware:

```bash
kindly-dedup deduplicate \
  --input data.jsonl \
  --output results.json \
  --gpu
```

**Supported GPUs**:
- NVIDIA (RTX 2000+, GTX 1600+)
- AMD (RX 5000+)
- Intel Arc (A-series)
- Apple Silicon (M1+)

**Expected Speedup**:
- Consumer GPUs: 2-4× faster
- Professional GPUs: 4-14× faster

### Threshold Selection

Choose threshold based on your use case:

| Threshold | Use Case | Behavior |
|-----------|----------|----------|
| 0.99-1.0 | Exact duplicates only | Very strict, only near-identical matches |
| 0.90-0.95 | High precision | Strict matching, low false positives |
| 0.80-0.90 | Balanced | Moderate matching, good precision/recall balance |
| 0.70-0.80 | High recall | Aggressive matching, may include false positives |
| < 0.70 | Not recommended | Too many false positives |

**Recommendation**: Start with 0.85, adjust based on results.

## Performance Tuning

### Thread Count

```bash
# Auto-detect (recommended)
kindly-dedup deduplicate --input data.jsonl --output results.json

# Manual override
kindly-dedup deduplicate --input data.jsonl --output results.json --threads 16
```

**Guidelines**:
- Use physical core count for CPU-bound workloads
- Use 2× core count for I/O-bound workloads
- Test to find optimal setting for your hardware

### Batch Size

```bash
kindly-dedup deduplicate --input data.jsonl --output results.json --batch-size 5000
```

**Tuning Guidelines**:
- Small datasets (< 100K): 1000-2000
- Medium datasets (100K-1M): 2000-5000
- Large datasets (> 1M): 5000-10000

**Trade-offs**:
- Larger batches: Better throughput, higher memory usage
- Smaller batches: Lower memory, more overhead

### Memory Management

Monitor memory usage and adjust:

```bash
# Low memory systems
kindly-dedup deduplicate \
  --input data.jsonl \
  --output results.json \
  --persistent \
  --batch-size 500

# High memory systems
kindly-dedup deduplicate \
  --input data.jsonl \
  --output results.json \
  --batch-size 10000
```

## Common Workflows

### Workflow 1: One-Time Deduplication

Process a static dataset once:

```bash
kindly-dedup deduplicate \
  --input raw_data.jsonl \
  --output clean_data.json \
  --threshold 0.85
```

### Workflow 2: Continuous Deduplication

Process growing dataset with daily updates:

```bash
# Initial load
kindly-dedup deduplicate \
  --input corpus_day1.jsonl \
  --output results_day1.json \
  --persistent \
  --storage-path ./storage

# Daily updates
kindly-dedup deduplicate \
  --input corpus_day2_new.jsonl \
  --output results_day2.json \
  --persistent \
  --storage-path ./storage \
  --incremental
```

### Workflow 3: Multi-Stage Processing

Process very large datasets in stages:

```bash
# Stage 1: Quick pass with high threshold
kindly-dedup deduplicate \
  --input huge_corpus.jsonl \
  --output stage1.json \
  --threshold 0.95 \
  --persistent

# Stage 2: Detailed pass on unique documents
kindly-dedup deduplicate \
  --input stage1_unique.jsonl \
  --output final.json \
  --threshold 0.85
```

## Examples

See [QUICK_START.md](QUICK_START.md) for basic examples.

## Troubleshooting

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for common issues and solutions.

## Support

For technical support, see [SUPPORT.md](SUPPORT.md).
